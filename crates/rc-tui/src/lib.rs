use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use rc_config::RuntimeConfig;
use rc_session::SessionStore;

pub fn run_dashboard(config: &RuntimeConfig, store: &SessionStore) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, config, store);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: &RuntimeConfig,
    store: &SessionStore,
) -> Result<()> {
    loop {
        let sessions = store.list_sessions().unwrap_or_default();
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Min(5)])
                .split(frame.area());

            let summary = Paragraph::new(format!(
                "Profile: {}\nProvider: {} ({})\nModel: {}\nPress q or Esc to exit.",
                config.paths.profile_dir.display(),
                config.provider.name,
                config.provider.protocol.as_str(),
                config.provider.model.as_deref().unwrap_or("(missing)")
            ))
            .block(
                Block::default()
                    .title("Remote Code Rust")
                    .borders(Borders::ALL)
                    .border_style(Style::default().add_modifier(Modifier::BOLD)),
            );
            frame.render_widget(summary, chunks[0]);

            let items = sessions
                .iter()
                .take(20)
                .map(|session| {
                    ListItem::new(format!(
                        "{}  {}  {}",
                        session.session_id, session.updated_at, session.title
                    ))
                })
                .collect::<Vec<_>>();
            let list = List::new(items).block(
                Block::default()
                    .title("Recent Sessions")
                    .borders(Borders::ALL),
            );
            frame.render_widget(list, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            break;
        }
    }
    Ok(())
}
