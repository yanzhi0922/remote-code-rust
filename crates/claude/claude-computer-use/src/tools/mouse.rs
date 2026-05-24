use anyhow::{Context, Result};
use enigo::{
    Button, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Mouse, Settings,
};
use serde_json::Value;
use std::thread;
use std::time::Duration;

fn parse_button(name: &str) -> Button {
    match name {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    }
}

pub async fn mouse_move(input: &Value) -> Result<String> {
    let x = input["x"].as_i64().context("missing x")? as i32;
    let y = input["y"].as_i64().context("missing y")? as i32;

    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.move_mouse(x, y, Coordinate::Abs)?;

    Ok(format!("moved mouse to ({x}, {y})"))
}

pub async fn mouse_click(input: &Value) -> Result<String> {
    let x = input["x"].as_i64().context("missing x")? as i32;
    let y = input["y"].as_i64().context("missing y")? as i32;
    let button_name = input["button"].as_str().unwrap_or("left");
    let double = input["double"].as_bool().unwrap_or(false);

    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.move_mouse(x, y, Coordinate::Abs)?;
    thread::sleep(Duration::from_millis(50));

    let btn = parse_button(button_name);
    if double {
        enigo.button(btn, Click)?;
        thread::sleep(Duration::from_millis(30));
        enigo.button(btn, Click)?;
    } else {
        enigo.button(btn, Click)?;
    }

    Ok(format!(
        "clicked {}{} at ({x}, {y})",
        button_name,
        if double { " (double)" } else { "" }
    ))
}

pub async fn mouse_drag(input: &Value) -> Result<String> {
    let from_x = input["from_x"].as_i64().context("missing from_x")? as i32;
    let from_y = input["from_y"].as_i64().context("missing from_y")? as i32;
    let to_x = input["to_x"].as_i64().context("missing to_x")? as i32;
    let to_y = input["to_y"].as_i64().context("missing to_y")? as i32;
    let button_name = input["button"].as_str().unwrap_or("left");

    let btn = parse_button(button_name);
    let mut enigo = Enigo::new(&Settings::default())?;

    enigo.move_mouse(from_x, from_y, Coordinate::Abs)?;
    thread::sleep(Duration::from_millis(50));
    enigo.button(btn, Press)?;
    thread::sleep(Duration::from_millis(100));

    enigo.move_mouse(to_x, to_y, Coordinate::Abs)?;
    thread::sleep(Duration::from_millis(50));
    enigo.button(btn, Release)?;

    Ok(format!(
        "dragged from ({from_x}, {from_y}) to ({to_x}, {to_y}) with {button_name} button"
    ))
}
