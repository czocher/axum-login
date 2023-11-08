use std::net::SocketAddr;

use axum::{error_handling::HandleErrorLayer, BoxError};
use axum_login::{
    permission_required,
    tower_sessions::{Expiry, MemoryStore, SessionManagerLayer},
    AuthManagerLayerBuilder,
};
use http::StatusCode;
use sqlx::SqlitePool;
use time::Duration;
use tower::ServiceBuilder;

use crate::{
    users::Backend,
    web::{auth, protected, restricted},
};

pub struct App {
    db: SqlitePool,
}

impl App {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let db = SqlitePool::connect(":memory:").await?;
        sqlx::migrate!().run(&db).await?;

        Ok(Self { db })
    }

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        // Session layer.
        //
        // This uses `tower-sessions` to establish a layer that will provide the session
        // as a request extension.
        let session_store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_expiry(Expiry::OnInactivity(Duration::days(1)));

        // Auth service.
        //
        // This combines the session layer with our backend to establish the auth
        // service which will provide the auth session as a request extension.
        let backend = Backend::new(self.db);
        let auth_service = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                StatusCode::BAD_REQUEST
            }))
            .layer(AuthManagerLayerBuilder::new(backend, session_layer).build());

        let app = restricted::router()
            .route_layer(permission_required!(
                Backend,
                login_url = "/login",
                "restricted.read",
            ))
            .merge(protected::router())
            .route_layer(permission_required!(
                Backend,
                login_url = "/login",
                "protected.read",
            ))
            .merge(auth::router())
            .layer(auth_service);

        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}
