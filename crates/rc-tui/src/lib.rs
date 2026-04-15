//! Interactive TUI for Remote Code Rust.
//!
//! Uses crossterm raw mode for true single-keystroke detection, enabling
//! real Vim keybindings (ESC, Ctrl+C, etc.) without line-buffered workarounds.
//!
//! Supports Vim key bindings:
//! - `h/j/k/l` — navigate message history (normal mode)
//! - `i` — enter insert (input) mode
//! - `Esc` — return to normal mode (real ESC detection via raw mode)
//! - `G` — jump to bottom of history
//! - `gg` — jump to top of history
//! - `:q` — quit
//! - `Ctrl+C` — exit from insert mode

mod commands;
mod completion;
mod slash_commands;
mod theme;

use std::io::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, StaticPermissionBroker, load_layered_rules,
};
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_provider::{ConversationBackend, ProviderClient, ProviderCompatBackend};
use rc_session::resume_state::{PendingToolCall, ResumeState};
use rc_session::{SessionStore, conversation::ensure_conversation_initialized};
use rc_tools::{
    ToolExecutionContext,
    agent::{parse_delegate_progress_event, render_delegate_progress_event},
    execute_tool_call,
};

use completion::{
    complete_slash_command, get_file_completions, get_tool_completions, update_search_results,
};
use slash_commands::{SlashCommandAction, handle_slash_command};
use theme::Theme;

/// Vim-like input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimMode {
    /// Normal mode: single-key commands for navigation.
    Normal,
    /// Insert mode: text input for conversation.
    Insert,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// RAII guard for terminal raw mode + alternate screen.
///
/// The TUI regularly suspends the raw terminal session while provider output is
/// printed. Keeping that lifecycle in one place ensures the terminal is
/// restored even if an error bubbles out in the middle of a turn.
struct TuiTerminalSession {
    active: bool,
}

impl TuiTerminalSession {
    fn enter() -> Result<Self> {
        let mut session = Self { active: false };
        session.activate()?;
        Ok(session)
    }

    fn activate(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        self.active = true;
        Ok(())
    }

    fn deactivate(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for TuiTerminalSession {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        self.active = false;
    }
}

/// Text-based dashboard that prints session info and recent sessions.
///
/// This is a non-interactive overview used by the `remote-code tui` subcommand.
pub fn run_dashboard(config: &RuntimeConfig, store: &SessionStore) -> Result<()> {
    println!("Remote Code Rust — Dashboard");
    println!();
    println!("Profile:  {}", config.paths.profile_dir.display());
    println!(
        "Provider: {} ({})",
        config.provider.name,
        config.provider.protocol.as_str()
    );
    println!(
        "Model:    {}",
        config.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!(
        "Base URL: {}",
        config.provider.base_url.as_deref().unwrap_or("(missing)")
    );
    println!(
        "API key:  {}",
        if config.provider.api_key.is_some() {
            "present"
        } else {
            "missing"
        }
    );
    println!();

    match store.list_sessions() {
        Ok(sessions) => {
            println!("Recent Sessions:");
            if sessions.is_empty() {
                println!("  (no sessions)");
            } else {
                for session in sessions.iter().take(20) {
                    println!(
                        "  {}  {}  {}",
                        session.session_id, session.updated_at, session.title
                    );
                }
            }
        }
        Err(error) => println!("  (error listing sessions: {error})"),
    }
    Ok(())
}

/// Run the interactive TUI application with crossterm raw-mode input.
///
/// This is the main interactive mode entry point, providing:
/// - Multi-turn conversation with the provider
/// - Automatic tool execution with permission checks
/// - Context window compaction
/// - Cost tracking across turns
/// - Slash commands for session management
/// - True Vim mode with raw key detection (ESC, Ctrl+C, etc.)
#[allow(clippy::too_many_lines)]
pub async fn run_tui_app(config: RuntimeConfig, store: &SessionStore) -> Result<()> {
    let provider_client = Arc::new(ProviderClient::new()?);
    let backend = ProviderCompatBackend::new(Arc::clone(&provider_client), &config.provider);
    let broker = LayeredPermissionBroker::new(
        StaticPermissionBroker::new(config.permission_mode),
        load_layered_rules(
            &config.cwd,
            &config.paths.profile_dir,
            &config.settings_files,
            &config.cli_settings_files,
        )?,
    );

    let model_name = config.provider.model.as_deref().unwrap_or("unknown");
    let context_manager = ContextWindowManager::for_model(model_name);
    let cost_tracker = CostTracker::new();
    let mut conversation = load_or_create_conversation(store, &config)?;

    let mut terminal_session = TuiTerminalSession::enter()?;

    // Helper to print a line in raw mode (use \r\n for line endings).
    let print_line = |text: &str| {
        let _ = crossterm::execute!(io::stdout(), crossterm::style::Print(format!("{text}\r\n")));
    };

    print_line("Remote Code Rust — Interactive Mode");
    print_line(&format!("Session:  {}", config.session_id));
    print_line(&format!(
        "Provider: {} ({})",
        config.provider.name,
        config.provider.protocol.as_str()
    ));
    print_line(&format!(
        "Model:    {}",
        config.provider.model.as_deref().unwrap_or("(missing)")
    ));
    print_line("Type /help for commands, /quit to exit");
    print_line("Vim mode: Esc=normal, i=insert, j/k=scroll, G=bottom, gg=top, :q=quit");
    print_line("");

    let mut vim_mode = VimMode::Insert;
    let mut history_scroll: usize = 0;
    let mut pending_g = false;
    let mut input_buffer = String::new();
    let mut cursor_pos: usize = 0; // Cursor position within input_buffer
    let mut command_buffer = String::new(); // for ':' commands in normal mode
    let mut input_history: Vec<String> = Vec::new(); // History of submitted inputs
    let mut history_index: usize = 0; // Current position in input history
    let mut saved_buffer: String = String::new(); // Saved buffer when navigating history
    let mut search_mode = false; // Ctrl+R reverse search active
    let mut search_query = String::new(); // Current search query
    let mut search_results: Vec<usize> = Vec::new(); // Matching history indices
    let mut search_result_index: usize = 0; // Current position in search results
    let mut theme = Theme::dark(); // Current color theme

    loop {
        // Print prompt and current input buffer.
        let prompt = if search_mode {
            format!("(search)'{}'> ", search_query)
        } else {
            match vim_mode {
                VimMode::Insert => "> ".to_owned(),
                VimMode::Normal => "(n) ".to_owned(),
            }
        };
        let display = format!("{prompt}{input_buffer}");
        let _ = crossterm::execute!(io::stdout(), crossterm::style::Print(&display));
        let _ = io::stdout().flush();

        // Poll for key events with a 100ms timeout.
        if !crossterm::event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        let event = match crossterm::event::read()? {
            crossterm::event::Event::Key(key) => key,
            crossterm::event::Event::Resize(_, _) => {
                // Redraw on resize — just continue the loop.
                continue;
            }
            _ => continue,
        };

        // Clear the current prompt line before processing.
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::cursor::MoveToColumn(0),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown)
        );

        match vim_mode {
            VimMode::Normal => {
                if pending_g {
                    pending_g = false;
                    if let crossterm::event::KeyCode::Char('g') = event.code {
                        // gg — jump to top
                        history_scroll = conversation.len();
                        print_line("  [top of history]");
                    }
                    continue;
                }

                match event.code {
                    crossterm::event::KeyCode::Char('i') | crossterm::event::KeyCode::Char('a') => {
                        vim_mode = VimMode::Insert;
                        input_buffer.clear();
                        print_line("  -- INSERT --");
                    }
                    crossterm::event::KeyCode::Char('j') => {
                        if history_scroll > 0 {
                            history_scroll -= 1;
                            let idx = conversation.len().saturating_sub(history_scroll + 1);
                            if let Some(entry) = conversation.get(idx) {
                                print_entry_raw(&print_line, entry);
                            }
                        } else {
                            print_line("  [at bottom]");
                        }
                    }
                    crossterm::event::KeyCode::Char('k') => {
                        if history_scroll < conversation.len() {
                            history_scroll += 1;
                            let idx = conversation.len().saturating_sub(history_scroll + 1);
                            if let Some(entry) = conversation.get(idx) {
                                print_entry_raw(&print_line, entry);
                            }
                        } else {
                            print_line("  [at top]");
                        }
                    }
                    crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Char('l') => {
                    }
                    crossterm::event::KeyCode::Char('G') => {
                        history_scroll = 0;
                        if let Some(entry) = conversation.last() {
                            print_entry_raw(&print_line, entry);
                        }
                    }
                    crossterm::event::KeyCode::Char('g') => {
                        pending_g = true;
                    }
                    crossterm::event::KeyCode::Char(':') => {
                        command_buffer.clear();
                        command_buffer.push(':');
                    }
                    crossterm::event::KeyCode::Char('q') => {
                        print_line("Goodbye!");
                        break;
                    }
                    crossterm::event::KeyCode::Esc => {
                        // Already in normal mode — ignore.
                    }
                    _ => {}
                }

                // Handle command buffer (e.g. ":q" entered char by char).
                if command_buffer.starts_with(':') {
                    match event.code {
                        crossterm::event::KeyCode::Char(c) => {
                            command_buffer.push(c);
                        }
                        crossterm::event::KeyCode::Enter => {
                            let cmd = command_buffer.trim();
                            if cmd == ":q" || cmd == ":quit" || cmd == ":exit" {
                                print_line("Goodbye!");
                                break;
                            }
                            command_buffer.clear();
                        }
                        crossterm::event::KeyCode::Esc => {
                            command_buffer.clear();
                        }
                        _ => {}
                    }
                }
                continue;
            }
            VimMode::Insert => {
                match event.code {
                    crossterm::event::KeyCode::Esc => {
                        if search_mode {
                            search_mode = false;
                            search_query.clear();
                            continue;
                        }
                        vim_mode = VimMode::Normal;
                        input_buffer.clear();
                        cursor_pos = 0;
                        print_line("  -- NORMAL --");
                        continue;
                    }
                    crossterm::event::KeyCode::Enter => {
                        // Exit search mode on Enter.
                        if search_mode {
                            search_mode = false;
                            search_query.clear();
                            continue;
                        }
                        // Shift+Enter: insert newline for multi-line input.
                        if event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT)
                        {
                            input_buffer.insert(cursor_pos, '\n');
                            cursor_pos += 1;
                            continue;
                        }

                        let input = input_buffer.trim().to_owned();
                        input_buffer.clear();
                        cursor_pos = 0;

                        // Save to input history (deduplicate, bounded to prevent unbounded growth).
                        const MAX_INPUT_HISTORY: usize = 1000;
                        if !input.is_empty() {
                            if input_history.last() != Some(&input) {
                                input_history.push(input.clone());
                                if input_history.len() > MAX_INPUT_HISTORY {
                                    input_history.remove(0);
                                }
                            }
                            history_index = input_history.len();
                            saved_buffer.clear();
                        }

                        // Temporarily leave raw mode for conversation turn output.
                        terminal_session.deactivate()?;

                        if input.is_empty() {
                            terminal_session.activate()?;
                            continue;
                        }

                        // Handle slash commands.
                        if input.starts_with('/') {
                            match handle_slash_command(
                                &input,
                                &config,
                                store,
                                &mut conversation,
                                &context_manager,
                                &cost_tracker,
                                &broker,
                                &mut theme,
                            ) {
                                SlashCommandAction::Quit => {
                                    let cost = cost_tracker.total_cost_usd();
                                    if cost > 0.0 {
                                        println!();
                                        print!("{}", cost_tracker.summary());
                                    }
                                    return Ok(());
                                }
                                SlashCommandAction::ResetScroll => history_scroll = 0,
                                SlashCommandAction::Continue => {}
                            }
                            terminal_session.activate()?;
                            continue;
                        }

                        // Execute conversation turn (outside raw mode for normal output).
                        if let Err(error) = run_conversation_turn(
                            &backend,
                            &config,
                            store,
                            &mut conversation,
                            &context_manager,
                            &broker,
                            &cost_tracker,
                            &input,
                        )
                        .await
                        {
                            let err_str = format!("{error:#}");
                            let is_transient = err_str.contains("timeout")
                                || err_str.contains("429")
                                || err_str.contains("rate limit")
                                || err_str.contains("503")
                                || err_str.contains("500")
                                || err_str.contains("connection");
                            if is_transient {
                                eprintln!("⚠ Transient error (recovered): {err_str}");
                                eprintln!(
                                    "  Your session is preserved. The next request will retry automatically."
                                );
                            } else {
                                eprintln!("⚠ Error: {err_str}");
                                eprintln!(
                                    "  Your message was saved. Type to continue or /help for options."
                                );
                            }
                        }

                        terminal_session.activate()?;
                        continue;
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        // Handle Ctrl+R: toggle reverse search mode.
                        if c == 'r'
                            && event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                        {
                            if search_mode {
                                // Already in search mode: cycle to next result.
                                if !search_results.is_empty() {
                                    search_result_index =
                                        (search_result_index + 1) % search_results.len();
                                    let idx = search_results[search_result_index];
                                    input_buffer = input_history[idx].clone();
                                    cursor_pos = input_buffer.len();
                                }
                            } else {
                                // Enter search mode.
                                search_mode = true;
                                search_query.clear();
                                search_results.clear();
                                search_result_index = 0;
                            }
                            continue;
                        }
                        // Handle Ctrl+C in insert mode.
                        if c == 'c'
                            && event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                        {
                            if search_mode {
                                search_mode = false;
                                search_query.clear();
                                continue;
                            }
                            if input_buffer.is_empty() {
                                print_line("Interrupted. Goodbye!");
                                break;
                            }
                            // Ctrl+C with text: clear the input buffer.
                            input_buffer.clear();
                            cursor_pos = 0;
                            print_line("  [input cleared — Ctrl+C again to exit]");
                            continue;
                        }
                        // In search mode, add to query and filter.
                        if search_mode {
                            search_query.push(c);
                            update_search_results(
                                &input_history,
                                &search_query,
                                &mut search_results,
                            );
                            search_result_index = 0;
                            if let Some(&idx) = search_results.first() {
                                input_buffer = input_history[idx].clone();
                                cursor_pos = input_buffer.len();
                            }
                            continue;
                        }
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                    crossterm::event::KeyCode::Backspace => {
                        if search_mode {
                            search_query.pop();
                            update_search_results(
                                &input_history,
                                &search_query,
                                &mut search_results,
                            );
                            search_result_index = 0;
                            if let Some(&idx) = search_results.first() {
                                input_buffer = input_history[idx].clone();
                                cursor_pos = input_buffer.len();
                            }
                            continue;
                        }
                        if cursor_pos > 0 && !input_buffer.is_empty() {
                            cursor_pos -= 1;
                            input_buffer.remove(cursor_pos);
                        }
                    }
                    crossterm::event::KeyCode::Delete => {
                        if cursor_pos < input_buffer.len() {
                            input_buffer.remove(cursor_pos);
                        }
                    }
                    crossterm::event::KeyCode::Left => {
                        cursor_pos = cursor_pos.saturating_sub(1);
                    }
                    crossterm::event::KeyCode::Right => {
                        if cursor_pos < input_buffer.len() {
                            cursor_pos += 1;
                        }
                    }
                    crossterm::event::KeyCode::Home => {
                        cursor_pos = 0;
                    }
                    crossterm::event::KeyCode::End => {
                        cursor_pos = input_buffer.len();
                    }
                    crossterm::event::KeyCode::Up => {
                        // Navigate input history (if not at first char, move up in conversation).
                        if cursor_pos == 0 && !input_history.is_empty() && history_index > 0 {
                            if history_index == input_history.len() {
                                saved_buffer = input_buffer.clone();
                            }
                            history_index -= 1;
                            input_buffer = input_history[history_index].clone();
                            cursor_pos = input_buffer.len();
                        } else if history_scroll < conversation.len() {
                            history_scroll += 1;
                        }
                    }
                    crossterm::event::KeyCode::Down => {
                        // Navigate input history forward.
                        if cursor_pos == 0 && history_index < input_history.len() {
                            history_index += 1;
                            if history_index == input_history.len() {
                                input_buffer = saved_buffer.clone();
                            } else {
                                input_buffer = input_history[history_index].clone();
                            }
                            cursor_pos = input_buffer.len();
                        } else if history_scroll > 0 {
                            history_scroll = history_scroll.saturating_sub(1);
                        }
                    }
                    crossterm::event::KeyCode::Tab => {
                        // Tab completion for slash commands, tool names, and file paths.
                        let completions = if input_buffer.starts_with('/') {
                            complete_slash_command(&input_buffer)
                        } else if input_buffer.is_empty() || input_buffer.ends_with(' ') {
                            // Suggest tool names at start or after space.
                            let tool_names = get_tool_completions("");
                            tool_names.into_iter().map(|t| format!("{t} ")).collect()
                        } else {
                            // Try to complete the last word as a tool name or file path.
                            let last_word = input_buffer.split_whitespace().last().unwrap_or("");
                            let mut results = Vec::new();
                            // Tool name completions.
                            results.extend(get_tool_completions(last_word));
                            // File path completions.
                            results.extend(get_file_completions(last_word, &config.cwd));
                            results
                        };
                        if completions.len() == 1 {
                            // Replace the last word with the completion.
                            if input_buffer.starts_with('/') {
                                input_buffer = completions[0].clone();
                            } else {
                                let last_space =
                                    input_buffer.rfind(' ').map(|i| i + 1).unwrap_or(0);
                                input_buffer.replace_range(last_space.., &completions[0]);
                            }
                            cursor_pos = input_buffer.len();
                        } else if !completions.is_empty() {
                            let display = completions.join("  ");
                            print_line(&format!("  {display}"));
                        }
                    }
                    _ => {}
                }
                continue;
            }
        }
    }

    terminal_session.deactivate()?;

    // Print cost summary on exit.
    let cost = cost_tracker.total_cost_usd();
    if cost > 0.0 {
        println!();
        print!("{}", cost_tracker.summary());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Conversation logic
// ---------------------------------------------------------------------------

/// Load an existing conversation or create a new one with a system prompt.
fn load_or_create_conversation(
    store: &SessionStore,
    config: &RuntimeConfig,
) -> Result<Vec<ConversationEntry>> {
    ensure_conversation_initialized(
        store,
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        config.session_name.as_deref(),
    )
}

/// Run a full multi-turn conversation turn.
///
/// This implements the core loop:
/// 1. Add user message
/// 2. Call provider
/// 3. If tool calls → execute tools → go to 2
/// 4. If no tool calls → display response → done
///
/// Tool execution errors are captured as error tool results rather than
/// propagating, ensuring the conversation state remains consistent for
/// the next provider call.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
async fn run_conversation_turn(
    backend: &dyn ConversationBackend,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    context_manager: &ContextWindowManager,
    broker: &dyn PermissionBroker,
    cost_tracker: &CostTracker,
    prompt: &str,
) -> Result<()> {
    // Add user message
    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(config.session_id, &user_entry)?;
    conversation.push(user_entry);

    let tool_context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
        sub_agent: Some(backend.sub_agent_completion()),
        progress_cb: Some(Arc::new(|msg: &str| {
            if let Some(event) = parse_delegate_progress_event(msg) {
                println!("{}", render_delegate_progress_event(&event));
            } else {
                println!("{msg}");
            }
        })),
        task_stack: std::sync::Arc::new(std::sync::Mutex::new(
            rc_core::task_stack::TaskStack::default(),
        )),
    };

    let model_name = config.provider.model.as_deref().unwrap_or("unknown");

    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;

    for turn in 0..config.max_turns {
        let budget_snapshot = context_manager.budget_snapshot(conversation);
        println!(
            "  [context] {:.0}% of {} tokens used (threshold {})",
            budget_snapshot.usage_ratio * 100.0,
            budget_snapshot.max_input_tokens,
            budget_snapshot.threshold_tokens(),
        );

        // Compact conversation if context window is getting full.
        if budget_snapshot.exceeds_threshold() {
            let compacted = context_manager.compact(conversation);
            let removed = conversation.len().saturating_sub(compacted.len());
            *conversation = compacted;
            if removed > 0 {
                let after = context_manager.budget_snapshot(conversation);
                println!(
                    "  [context compacted: {removed} entries summarized, now {:.0}%]",
                    after.usage_ratio * 100.0
                );
            }
        }

        // Call provider
        let response = backend.complete(conversation).await?;
        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        // Record usage in cost tracker
        cost_tracker.record(
            model_name,
            response.usage.input_tokens,
            response.usage.output_tokens,
        );

        // Build and persist assistant entry
        let assistant_entry = ConversationEntry {
            role: ConversationRole::Assistant,
            text: response.text.clone(),
            history_text: response.history_text.clone(),
            content_blocks: response.content_blocks.clone(),
            tool_calls: response.tool_calls.clone(),
            attachments: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        };
        store.append_conversation_entry(config.session_id, &assistant_entry)?;
        conversation.push(assistant_entry);

        // If no tool calls, display the response and finish
        if response.tool_calls.is_empty() {
            store.clear_resume_state(config.session_id)?;
            println!();
            println!("{}", response.text);
            println!(
                "-- turn {}, {} input tokens, {} output tokens, stop={}",
                turn + 1,
                total_input_tokens,
                total_output_tokens,
                response.stop_reason,
            );
            return Ok(());
        }

        let pending_tool_calls = response
            .tool_calls
            .iter()
            .map(|tool_call| PendingToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                input: tool_call.input.clone(),
            })
            .collect::<Vec<_>>();
        store.save_resume_state(
            config.session_id,
            &ResumeState::from_pending_calls(pending_tool_calls),
        )?;

        // Execute tool calls
        println!();
        if !response.text.is_empty() {
            println!("{}", response.text);
        }

        for tool_call in &response.tool_calls {
            let tool_start = std::time::Instant::now();
            println!("  ⏳ [tool] {} — running...", tool_call.name);
            let audit_count_before = broker.audit_records().len();

            // Capture tool execution errors as error tool results instead of
            // propagating, to keep conversation state consistent for the next
            // provider call.
            let tool_result = match execute_tool_call(tool_call, &tool_context, broker).await {
                Ok(result) => result,
                Err(error) => {
                    let elapsed = tool_start.elapsed();
                    eprintln!(
                        "  ✗ [tool] {} — error ({:.1}s): {error}",
                        tool_call.name,
                        elapsed.as_secs_f64()
                    );
                    rc_core::ToolResult {
                        content: format!("Tool execution error: {error}"),
                        is_error: true,
                    }
                }
            };
            let new_audits = broker
                .audit_records()
                .into_iter()
                .skip(audit_count_before)
                .collect::<Vec<_>>();
            for audit in new_audits {
                store.append_named_event(
                    config.session_id,
                    "permission_decision",
                    serde_json::to_value(&audit)?,
                )?;
            }
            let elapsed = tool_start.elapsed();
            let status = if tool_result.is_error { "✗" } else { "✓" };
            println!(
                "  {status} [tool] {} — done ({:.1}s)",
                tool_call.name,
                elapsed.as_secs_f64()
            );

            // Truncate tool output for context management
            let truncated_output =
                context_manager.truncate_tool_output_default(&tool_result.content);

            print_tool_result(&tool_call.name, &tool_result, &truncated_output);

            let tool_entry = ConversationEntry::tool(
                tool_call.id.clone(),
                tool_call.name.clone(),
                truncated_output,
                tool_result.is_error,
            );
            store.append_conversation_entry(config.session_id, &tool_entry)?;
            conversation.push(tool_entry);
        }
        store.clear_resume_state(config.session_id)?;
    }

    eprintln!(
        "Maximum turn budget reached ({}) without a final assistant reply.",
        config.max_turns
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Print a conversation entry for Vim-mode history navigation (raw mode).
fn print_entry_raw(print_line: &dyn Fn(&str), entry: &ConversationEntry) {
    let role = match entry.role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    };
    let text = entry.history_text();
    let preview: String = text.chars().take(200).collect();
    print_line(&format!("  [{role}] {preview}"));
}

/// Format and print a tool execution result.
fn print_tool_result(tool_name: &str, result: &rc_core::ToolResult, display_text: &str) {
    if result.is_error {
        println!(
            "  [tool] {tool_name} — ERROR: {}",
            truncate_display(display_text, 200)
        );
    } else {
        println!("  [tool] {tool_name} — OK");
        // Show first few lines of output
        for line in display_text.lines().take(5) {
            println!("    {}", truncate_display(line, 120));
        }
        let total_lines = display_text.lines().count();
        if total_lines > 5 {
            println!("    ... ({} more lines)", total_lines - 5);
        }
    }
}

/// Truncate a string for display purposes.
fn truncate_display(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}
