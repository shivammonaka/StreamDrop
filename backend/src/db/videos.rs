use crate::models::video::{Video, VideoStatus};
use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

/// Insert a new video record when upload begins.
pub async fn create(
    pool: &PgPool,
    slug: &str,
    original_path: &str,
    size_bytes: i64,
    mime_type: &str,
) -> Result<Video> {
    let video = sqlx::query_as::<_, Video>(
        r#"
        INSERT INTO videos (slug, status, original_path, size_bytes, mime_type)
        VALUES ($1, 'Pending', $2, $3, $4)
        RETURNING id, slug, status, original_path, hls_path, size_bytes, mime_type, created_at
        "#,
    )
    .bind(slug)
    .bind(original_path)
    .bind(size_bytes)
    .bind(mime_type)
    .fetch_one(pool)
    .await
    .context("Failed to insert video record")?;

    Ok(video)
}

/// Fetch a video by its public slug.
pub async fn get_by_slug(pool: &PgPool, slug: &str) -> Result<Video> {
    let video = sqlx::query_as::<_, Video>(
        r#"
        SELECT id, slug, status, original_path, hls_path, size_bytes, mime_type, created_at
        FROM videos
        WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_one(pool)
    .await
    .context(format!("Video not found with slug: {}", slug))?;

    Ok(video)
}

/// Fetch a video by its internal UUID.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Video> {
    let video = sqlx::query_as::<_, Video>(
        r#"
        SELECT id, slug, status, original_path, hls_path, size_bytes, mime_type, created_at
        FROM videos
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context(format!("Video not found with id: {}", id))?;

    Ok(video)
}

/// Update video status and optionally set the HLS manifest path.
pub async fn update_status(
    pool: &PgPool,
    id: Uuid,
    status: VideoStatus,
    hls_path: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE videos
        SET status = $1, hls_path = $2
        WHERE id = $3
        "#,
    )
    .bind(status.to_string())
    .bind(hls_path)
    .bind(id)
    .execute(pool)
    .await
    .context(format!("Failed to update status for video: {}", id))?;

    Ok(())
}