use ansi_to_tui::IntoText;
use ratatui::layout::Alignment;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui_core::layout as core_layout;
use ratatui_core::style as core_style;
use ratatui_core::text as core_text;

// Expand tabs in a best-effort way for transcript rendering.
// Tabs can interact poorly with left-gutter prefixes in our TUI and CLI
// transcript views (e.g., `nl` separates line numbers from content with a tab).
// Replacing tabs with spaces avoids odd visual artifacts without changing
// semantics for our use cases.
fn expand_tabs(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\t') {
        // Keep it simple: replace each tab with 4 spaces.
        // We do not try to align to tab stops since most usages (like `nl`)
        // look acceptable with a fixed substitution and this avoids stateful math
        // across spans.
        std::borrow::Cow::Owned(s.replace('\t', "    "))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// This function should be used when the contents of `s` are expected to match
/// a single line. If multiple lines are found, a warning is logged and only the
/// first line is returned.
pub fn ansi_escape_line(s: &str) -> Line<'static> {
    // Normalize tabs to spaces to avoid odd gutter collisions in transcript mode.
    let s = expand_tabs(s);
    let text = ansi_escape(&s);
    match text.lines.as_slice() {
        [] => "".into(),
        [only] => only.clone(),
        [first, rest @ ..] => {
            tracing::warn!("ansi_escape_line: expected a single line, got {first:?} and {rest:?}");
            first.clone()
        }
    }
}

pub fn ansi_escape(s: &str) -> Text<'static> {
    // to_text() claims to be faster, but introduces complex lifetime issues
    // such that it's not worth it.
    match s.into_text() {
        Ok(text) => convert_text(text),
        Err(err) => {
            tracing::warn!("ansi_to_tui parse error, falling back to raw text: {err}");
            Text::raw(s.to_owned())
        }
    }
}

fn convert_text(text: core_text::Text<'static>) -> Text<'static> {
    Text {
        alignment: text.alignment.map(convert_alignment),
        style: convert_style(text.style),
        lines: text.lines.into_iter().map(convert_line).collect(),
    }
}

fn convert_line(line: core_text::Line<'static>) -> Line<'static> {
    Line {
        style: convert_style(line.style),
        alignment: line.alignment.map(convert_alignment),
        spans: line.spans.into_iter().map(convert_span).collect(),
    }
}

fn convert_span(span: core_text::Span<'static>) -> Span<'static> {
    Span {
        style: convert_style(span.style),
        content: span.content,
    }
}

fn convert_style(style: core_style::Style) -> Style {
    let mut result = Style::default();

    if let Some(fg) = style.fg {
        result = result.fg(convert_color(fg));
    }

    if let Some(bg) = style.bg {
        result = result.bg(convert_color(bg));
    }

    result = result.add_modifier(convert_modifier(style.add_modifier));
    result.remove_modifier(convert_modifier(style.sub_modifier))
}

/// `ansi-to-tui` preserves ANSI 256-color and true-color escape sequences as
/// `Indexed`/`Rgb`. This bridge keeps that parsed terminal output intact.
#[allow(clippy::disallowed_methods)]
fn convert_color(color: core_style::Color) -> Color {
    match color {
        core_style::Color::Reset => Color::Reset,
        core_style::Color::Black => Color::Black,
        core_style::Color::Red => Color::Red,
        core_style::Color::Green => Color::Green,
        core_style::Color::Yellow => Color::Yellow,
        core_style::Color::Blue => Color::Blue,
        core_style::Color::Magenta => Color::Magenta,
        core_style::Color::Cyan => Color::Cyan,
        core_style::Color::Gray => Color::Gray,
        core_style::Color::DarkGray => Color::DarkGray,
        core_style::Color::LightRed => Color::LightRed,
        core_style::Color::LightGreen => Color::LightGreen,
        core_style::Color::LightYellow => Color::LightYellow,
        core_style::Color::LightBlue => Color::LightBlue,
        core_style::Color::LightMagenta => Color::LightMagenta,
        core_style::Color::LightCyan => Color::LightCyan,
        core_style::Color::White => Color::White,
        core_style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        core_style::Color::Indexed(index) => Color::Indexed(index),
    }
}

fn convert_alignment(alignment: core_layout::Alignment) -> Alignment {
    match alignment {
        core_layout::Alignment::Left => Alignment::Left,
        core_layout::Alignment::Center => Alignment::Center,
        core_layout::Alignment::Right => Alignment::Right,
    }
}

fn convert_modifier(modifier: core_style::Modifier) -> Modifier {
    let mut result = Modifier::empty();

    if modifier.contains(core_style::Modifier::BOLD) {
        result |= Modifier::BOLD;
    }
    if modifier.contains(core_style::Modifier::DIM) {
        result |= Modifier::DIM;
    }
    if modifier.contains(core_style::Modifier::ITALIC) {
        result |= Modifier::ITALIC;
    }
    if modifier.contains(core_style::Modifier::UNDERLINED) {
        result |= Modifier::UNDERLINED;
    }
    if modifier.contains(core_style::Modifier::SLOW_BLINK) {
        result |= Modifier::SLOW_BLINK;
    }
    if modifier.contains(core_style::Modifier::RAPID_BLINK) {
        result |= Modifier::RAPID_BLINK;
    }
    if modifier.contains(core_style::Modifier::REVERSED) {
        result |= Modifier::REVERSED;
    }
    if modifier.contains(core_style::Modifier::HIDDEN) {
        result |= Modifier::HIDDEN;
    }
    if modifier.contains(core_style::Modifier::CROSSED_OUT) {
        result |= Modifier::CROSSED_OUT;
    }

    result
}
