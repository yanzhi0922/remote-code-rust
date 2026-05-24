use std::env;
use std::io::Cursor;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use uuid::Uuid;
use xcap::Monitor;

pub async fn screenshot(input: &Value) -> Result<String> {
    let monitor_idx = input["monitor"].as_u64().unwrap_or(0) as usize;

    let monitors = Monitor::all().context("failed to enumerate monitors")?;
    if monitor_idx >= monitors.len() {
        return Err(anyhow!(
            "monitor index {monitor_idx} out of range (0-{})",
            monitors.len() - 1
        ));
    }
    let monitor = &monitors[monitor_idx];

    let image = monitor.capture_image().context("screen capture failed")?;
    let width = image.width();
    let height = image.height();

    let screenshots_dir = env::temp_dir()
        .join("remote-code-rust")
        .join("computer-use-screenshots");
    std::fs::create_dir_all(&screenshots_dir)?;

    let filename = format!("{}.png", Uuid::new_v4());
    let path = screenshots_dir.join(&filename);

    let mut buf = Cursor::new(Vec::new());
    image.write_to(&mut buf, image::ImageFormat::Png)?;
    std::fs::write(&path, buf.into_inner())?;

    Ok(json!({
        "type": "computer_use_screenshot",
        "path": path.to_string_lossy(),
        "mime_type": "image/png",
        "width": width,
        "height": height,
        "monitor": monitor_idx,
    })
    .to_string())
}

pub async fn get_screen_size(_input: &Value) -> Result<String> {
    let monitors = Monitor::all().context("failed to enumerate monitors")?;
    let primary = monitors.first().context("no monitors found")?;

    let width = primary.width().context("failed to get primary width")?;
    let height = primary.height().context("failed to get primary height")?;

    let mut details = Vec::new();
    for m in &monitors {
        details.push(json!({
            "name": m.name().unwrap_or_default(),
            "width": m.width().unwrap_or(0),
            "height": m.height().unwrap_or(0),
            "x": m.x().unwrap_or(0),
            "y": m.y().unwrap_or(0),
        }));
    }

    Ok(json!({
        "width": width,
        "height": height,
        "monitors": monitors.len(),
        "monitor_details": details,
    })
    .to_string())
}
