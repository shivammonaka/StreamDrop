use anyhow::Result;
use sqlx::PgPool;
use tokio::process::Command;
use tokio::fs;
use uuid::Uuid;
use crate::db;
use crate::models::video::VideoStatus;

pub async fn run(
    video_id: Uuid,
    input_path: &str,
    output_dir: &str,
    pool: &PgPool,
) {
    // update status to Processing
    db::videos::update_status(pool, video_id, VideoStatus::Processing, None)
        .await
        .unwrap();

    println!("🎬 Starting transcoding for video: {}", video_id);

    match transcode(input_path, output_dir).await {
        Ok(_) => {
            // ffmpeg succeeded, update status to Ready
            let hls_path = format!("{}/index.m3u8", output_dir);
            println!("✅ Transcoding complete: {}", hls_path);

            db::videos::update_status(
                pool,
                video_id,
                VideoStatus::Ready,
                Some(hls_path),
            )
            .await
            .unwrap();
        }
        Err(e) => {
            // ffmpeg failed, update status to Failed
            println!("❌ Transcoding failed: {}", e);

            db::videos::update_status(pool, video_id, VideoStatus::Failed, None)
                .await
                .unwrap();
        }
    }
}

async fn transcode(input_path: &str, output_dir: &str) -> Result<()> {
    // create output folder if it doesnt exist
    fs::create_dir_all(output_dir).await?;

    let output_path = format!("{}/index.m3u8", output_dir);

    // run ffmpeg
    let status = Command::new("ffmpeg")
        .arg("-loglevel").arg("error")
        .args([
            "-i", input_path,       // input file
            "-codec:", "copy",      // copy video/audio as-is (fast, no re-encoding)
            "-start_number", "0",   // start chunk numbering from 0
            "-hls_time", "10",      // each chunk is 10 seconds
            "-hls_list_size", "0",  // keep all chunks in manifest
            "-f", "hls",            // output format is HLS
            &output_path,           // where to write index.m3u8
        ])
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("ffmpeg exited with status: {}", status))
    }
}