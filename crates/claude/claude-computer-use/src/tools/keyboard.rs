use anyhow::Result;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use serde_json::Value;

fn parse_key(name: &str) -> Key {
    match name.to_lowercase().as_str() {
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page_up" => Key::PageUp,
        "pagedown" | "page_down" => Key::PageDown,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "space" => Key::Space,
        "ctrl" | "control" => Key::Control,
        "alt" => Key::Alt,
        "shift" => Key::Shift,
        "meta" | "super" | "win" | "cmd" | "command" => Key::Meta,
        "capslock" => Key::CapsLock,
        s if s.len() == 1 => Key::Unicode(s.chars().next().unwrap()),
        _ => Key::Unicode(name.chars().next().unwrap()),
    }
}

pub async fn type_text(input: &Value) -> Result<String> {
    let text = input["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing text"))?;

    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.text(text)?;

    Ok(format!("typed {} characters", text.len()))
}

pub async fn key_press(input: &Value) -> Result<String> {
    let key_str = input["key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing key"))?;
    let count = input["count"].as_u64().unwrap_or(1) as usize;

    let mut enigo = Enigo::new(&Settings::default())?;

    let parts: Vec<&str> = key_str.split('+').collect();

    if parts.len() > 1 {
        let modifiers: Vec<Key> = parts[..parts.len() - 1]
            .iter()
            .map(|p| parse_key(p.trim()))
            .collect();
        let main_key = parse_key(parts.last().unwrap().trim());

        for _ in 0..count {
            for m in &modifiers {
                enigo.key(*m, Press)?;
            }
            enigo.key(main_key, Click)?;
            for m in modifiers.iter().rev() {
                enigo.key(*m, Release)?;
            }
        }
    } else {
        let key = parse_key(key_str);
        for _ in 0..count {
            enigo.key(key, Click)?;
        }
    }

    Ok(format!(
        "pressed \"{key_str}\"{extra}",
        extra = if count > 1 {
            format!(" {count}x")
        } else {
            String::new()
        }
    ))
}
