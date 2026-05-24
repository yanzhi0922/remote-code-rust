use anyhow::{Context, Result};
use enigo::{Axis::Vertical, Coordinate, Enigo, Mouse, Settings};
use serde_json::Value;
use std::thread;
use std::time::Duration;

pub async fn scroll(input: &Value) -> Result<String> {
    let x = input["x"].as_i64().context("missing x")? as i32;
    let y = input["y"].as_i64().context("missing y")? as i32;
    let direction = input["direction"].as_str().unwrap_or("down");
    let amount = input["amount"].as_i64().unwrap_or(3) as i32;

    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.move_mouse(x, y, Coordinate::Abs)?;
    thread::sleep(Duration::from_millis(30));

    let scroll_amount = if direction == "up" { -amount } else { amount };
    enigo.scroll(scroll_amount, Vertical)?;

    Ok(format!("scrolled {direction} {amount} at ({x}, {y})"))
}
