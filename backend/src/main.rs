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

// AppState is passed into every route handler
// like Spring's ApplicationContext
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub storage: Arc<dyn storage::StorageBackend>,
}

#[tokio::main]
async fn main() {
    // load .env file
    dotenv().ok();

    // connect to database
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    println!("✅ Connected to database");

    // set up local storage
    let storage = storage::local::LocalStorage {
        base_path: "./storage".to_string(),
    };

    // build app state
    let state = AppState {
        db: pool,
        storage: Arc::new(storage),
    };

    // set up routes
    let app = Router::new()
        .route("/api/videos", post(routes::videos::upload))
        .route("/api/videos/:id/status", get(routes::videos::status))
        .route("/v/:slug", get(routes::stream::get_video))
        .route("/videos/:slug/*file", get(routes::stream::serve_file))
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive())
        .with_state(state);

    // start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("🚀 Server running on http://localhost:3000");

    axum::serve(listener, app).await.unwrap();
}