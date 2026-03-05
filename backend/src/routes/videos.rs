use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;
use nanoid::nanoid;
use tokio::io::AsyncWriteExt;
use crate::AppState;
use crate::db;
use crate::services::transcode;

const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024; // 1GB

#[derive(Serialize)]
pub struct UploadResponse {
    pub video_id: String,
    pub slug: String,
    pub share_url: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub manifest_url: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// POST /api/videos
/// Accepts video upload, streams to disk, triggers transcoding in background.
/// Returns share URL immediately without waiting for transcoding.
pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {

    let slug = nanoid!(8);
    let mut saved_path = String::new();
    let mut total_bytes: u64 = 0;
    let mut mime_type = String::from("video/mp4");

    // get upload directory from storage backend
    let upload_dir = state.storage.upload_path();
    if let Err(e) = tokio::fs::create_dir_all(&upload_dir).await {
        tracing::error!("Failed to create upload dir: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Storage error".to_string() }),
        ).into_response();
    }

    // stream file to disk chunk by chunk — never loads full file into RAM
    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field.file_name()
            .unwrap_or("video.mp4")
            .to_string();

        mime_type = field.content_type()
            .unwrap_or("video/mp4")
            .to_string();

        // reject non-video files
        if !mime_type.starts_with("video/") {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "Only video files are allowed".to_string() }),
            ).into_response();
        }

        saved_path = format!("{}/{}-{}", upload_dir, slug, filename);

        let mut file = match tokio::fs::File::create(&saved_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create file: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: "Failed to save file".to_string() }),
                ).into_response();
            }
        };

        let mut stream = field;
        while let Ok(Some(chunk)) = stream.chunk().await {
            total_bytes += chunk.len() as u64;

            // enforce 1GB limit
            if total_bytes > MAX_FILE_SIZE {
                tokio::fs::remove_file(&saved_path).await.ok();
                return (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(ErrorResponse { error: "File too large, max 1GB".to_string() }),
                ).into_response();
            }

            if let Err(e) = file.write_all(&chunk).await {
                tracing::error!("Failed to write chunk: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: "Failed to write file".to_string() }),
                ).into_response();
            }
        }
    }

    tracing::info!("Received {} bytes for slug {}", total_bytes, slug);

    // save metadata to database
    let video = match db::videos::create(
        &state.db,
        &slug,
        &saved_path,
        total_bytes as i64,
        &mime_type,
    ).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to create video record: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Database error".to_string() }),
            ).into_response();
        }
    };

    // trigger transcoding in background
    // semaphore limits concurrent ffmpeg processes to MAX_TRANSCODING_JOBS
    let db_pool = state.db.clone();
    let video_id = video.id;
    let saved_path_clone = saved_path.clone();
    let hls_path = state.storage.hls_output_path(&slug);
    let semaphore = state.transcode_semaphore.clone();
    tokio::spawn(async move {
        // wait for a slot to be available
        let _permit = semaphore.acquire_owned().await.unwrap();
        // _permit dropped at end of block → releases slot for next job
        transcode::run(video_id, &saved_path_clone, &hls_path, &db_pool).await;
    });

    // return immediately, user gets share URL before transcoding finishes
    Json(UploadResponse {
        video_id: video.id.to_string(),
        slug: slug.clone(),
        share_url: format!("http://localhost:5173/v/{}", slug),
    }).into_response()
}

/// GET /api/videos/:id/status
/// Returns current transcoding status and manifest URL when ready.
pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {

    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Invalid video ID".to_string() }),
        ).into_response(),
    };

    match db::videos::get_by_id(&state.db, uuid).await {
        Ok(video) => Json(StatusResponse {
            status: video.status.to_string(),
            manifest_url: video.hls_path
                .map(|p| state.storage.public_url(&p)),
        }).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "Video not found".to_string() }),
        ).into_response(),
    }
}