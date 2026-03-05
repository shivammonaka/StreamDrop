use axum::{routing::{get, post}, Router};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use dotenvy::dotenv;
use std::env;

mod models;
mod db;
mod storage;
mod routes;
mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub storage: Arc<dyn storage::StorageBackend>,
    pub transcode_semaphore: Arc<tokio::sync::Semaphore>,
}

#[tokio::main]
async fn main() {
    // initialize logging first so we can see all logs
    tracing_subscriber::fmt::init();

    // load .env file
    dotenv().ok();

    // read config from environment
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let storage_path = env::var("STORAGE_PATH").unwrap_or_else(|_| "./storage".to_string());
    let max_connections = env::var("MAX_DB_CONNECTIONS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<u32>()
        .expect("MAX_DB_CONNECTIONS must be a number");

    // connect to database
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    tracing::info!("Connected to database");

    // set up storage backend
    // swap LocalStorage for S3Storage here when moving to production
    let storage = storage::local::LocalStorage::new(
    &storage_path,
    &format!("http://localhost:{}", port),
);

    let max_transcode_jobs = env::var("MAX_TRANSCODING_JOBS")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<usize>()
        .expect("MAX_TRANSCODING_JOBS must be a number");

    let state = AppState {
        db: pool,
        storage: Arc::new(storage),
        transcode_semaphore: Arc::new(tokio::sync::Semaphore::new(max_transcode_jobs)),
    };

    let app = Router::new()
        .route("/api/videos", post(routes::videos::upload))
        .route("/api/videos/:id/status", get(routes::videos::status))
        .route("/v/:slug", get(routes::stream::get_video))
        .route("/videos/:slug/*file", get(routes::stream::serve_file))
        .route("/health", get(routes::health::health_check))
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive()) // TODO: restrict to frontend URL in production
        .with_state(state);

    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect(&format!("Failed to bind to {}", bind_addr));

    tracing::info!("Server running on http://localhost:{}", port);

    axum::serve(listener, app).await
        .expect("Server crashed unexpectedly");
}