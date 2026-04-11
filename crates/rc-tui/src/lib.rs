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

use std::io::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use rc_config::{ProviderConfig, RuntimeConfig};
use rc_core::{ConversationEntry, ConversationRole, ProviderResponse, SubAgentCompletion, default_system_prompt};
use rc_permissions::StaticPermissionBroker;
use rc_provider::ProviderClient;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};

/// Vim-like input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimMode {
    /// Normal mode: single-key commands for navigation.
    Normal,
    /// Insert mode: text input for conversation.
    Insert,
}

/// Result of handling a slash command.
enum SlashCommandAction {
    /// Continue the input loop normally.
    Continue,
    /// Reset history scroll position (e.g. after /clear).
    ResetScroll,
    /// Exit the interactive session.
    Quit,
}

// ---------------------------------------------------------------------------
// Sub-agent completion provider
// ---------------------------------------------------------------------------

/// Wrapper around [`ProviderClient`] that implements [`SubAgentCompletion`].
///
/// This allows the agent tool to create sub-conversations and execute them
/// using the same provider configuration as the main conversation.
struct TuiSubAgent {
    client: ProviderClient,
    provider: ProviderConfig,
}

impl TuiSubAgent {
    fn new(client: &ProviderClient, provider: &ProviderConfig) -> Self {
        Self {
            client: client.clone(),
            provider: provider.clone(),
        }
    }
}

#[async_trait::async_trait]
impl SubAgentCompletion for TuiSubAgent {
    async fn complete(
        &self,
        conversation: &[ConversationEntry],
    ) -> anyhow::Result<ProviderResponse> {
        self.client.complete(&self.provider, conversation).await
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Text-based dashboard that prints session info and recent sessions.
///
/// This is a non-interactive overview used by the `remote-code tui` subcommand.
pub fn run_dashboard(config: &RuntimeConfig, store: &SessionStore) -> Result<()> {
    println!("Remote Code Rust — Dashboard");
    println!();
    println!("Profile:  {}", config.paths.profile_dir.display());
    println!("Provider: {} ({})", config.provider.name, config.provider.protocol.as_str());
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
pub async fn run_tui_app(
    config: RuntimeConfig,
    store: &SessionStore,
) -> Result<()> {
    let provider = ProviderClient::new()?;
    let broker = StaticPermissionBroker::new(config.permission_mode);

    let model_name = config.provider.model.as_deref().unwrap_or("unknown");
    let context_manager = ContextWindowManager::for_model(model_name);
    let cost_tracker = CostTracker::new();
    let mut conversation = load_or_create_conversation(store, &config)?;

    // Enter crossterm alternate screen and raw mode.
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

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

    // Enable mouse capture for scroll support.
    let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture);

    loop {
        // Print prompt and current input buffer.
        let prompt = match vim_mode {
            VimMode::Insert => "> ",
            VimMode::Normal => "(n) ",
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
                    crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Char('l') => {}
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
                        vim_mode = VimMode::Normal;
                        input_buffer.clear();
                        cursor_pos = 0;
                        print_line("  -- NORMAL --");
                        continue;
                    }
                    crossterm::event::KeyCode::Enter => {
                        // Shift+Enter: insert newline for multi-line input.
                        if event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            input_buffer.insert(cursor_pos, '\n');
                            cursor_pos += 1;
                            continue;
                        }

                        let input = input_buffer.trim().to_owned();
                        input_buffer.clear();
                        cursor_pos = 0;

                        // Save to input history (deduplicate).
                        if !input.is_empty() {
                            if input_history.last() != Some(&input) {
                                input_history.push(input.clone());
                            }
                            history_index = input_history.len();
                            saved_buffer.clear();
                        }

                        // Temporarily leave raw mode for conversation turn output.
                        crossterm::terminal::disable_raw_mode()?;
                        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;

                        if input.is_empty() {
                            // Re-enter raw mode.
                            crossterm::terminal::enable_raw_mode()?;
                            crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
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
                            ) {
                                SlashCommandAction::Quit => {
                                    // Disable mouse capture before exit.
                                    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
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
                            // Re-enter raw mode.
                            crossterm::terminal::enable_raw_mode()?;
                            crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
                            continue;
                        }

                        // Execute conversation turn (outside raw mode for normal output).
                        if let Err(error) = run_conversation_turn(
                            &provider,
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
                            eprintln!("Error: {error}");
                            eprintln!("  (your message was saved; the conversation state may be inconsistent)");
                        }

                        // Re-enter raw mode.
                        crossterm::terminal::enable_raw_mode()?;
                        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
                        continue;
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        // Handle Ctrl+C in insert mode.
                        if c == 'c' && event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
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
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                    crossterm::event::KeyCode::Backspace => {
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
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                        }
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
                        // Tab completion for slash commands.
                        if input_buffer.starts_with('/') {
                            let completions = complete_slash_command(&input_buffer);
                            if completions.len() == 1 {
                                input_buffer = completions[0].clone();
                                cursor_pos = input_buffer.len();
                            } else if !completions.is_empty() {
                                // Show available completions.
                                let display = completions.join("  ");
                                print_line(&format!("  {display}"));
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
        }
    }

    // Restore terminal state.
    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;

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
    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        None, // title_hint: let the store generate a default title
    )?;

    let mut conversation = store.load_conversation(config.session_id).unwrap_or_default();

    if conversation.is_empty() {
        let system = ConversationEntry::system(default_system_prompt(&config.cwd));
        store.append_conversation_entry(config.session_id, &system)?;
        conversation.push(system);
    }

    Ok(conversation)
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
    provider: &ProviderClient,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    context_manager: &ContextWindowManager,
    broker: &StaticPermissionBroker,
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
        sub_agent: Some(Arc::new(TuiSubAgent::new(provider, &config.provider))),
    };

    let model_name = config
        .provider
        .model
        .as_deref()
        .unwrap_or("unknown");

    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;

    for turn in 0..config.max_turns {
        // Compact conversation if context window is getting full
        if context_manager.needs_compaction(conversation) {
            let compacted = context_manager.compact(conversation);
            let removed = conversation.len().saturating_sub(compacted.len());
            *conversation = compacted;
            if removed > 0 {
                println!("  [context compacted: {removed} entries summarized]");
            }
        }

        // Call provider
        let response = provider.complete(&config.provider, conversation).await?;
        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        // Record usage in cost tracker
        cost_tracker.record(model_name, response.usage.input_tokens, response.usage.output_tokens);

        // Build and persist assistant entry
        let assistant_entry = ConversationEntry {
            role: ConversationRole::Assistant,
            text: response.text.clone(),
            history_text: response.history_text.clone(),
            content_blocks: response.content_blocks.clone(),
            tool_calls: response.tool_calls.clone(),
            tool_call_id: None,
            name: None,
            is_error: false,
        };
        store.append_conversation_entry(config.session_id, &assistant_entry)?;
        conversation.push(assistant_entry);

        // If no tool calls, display the response and finish
        if response.tool_calls.is_empty() {
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

        // Execute tool calls
        println!();
        if !response.text.is_empty() {
            println!("{}", response.text);
        }

        for tool_call in &response.tool_calls {
            println!("  [tool] {} ...", tool_call.name);

            // Capture tool execution errors as error tool results instead of
            // propagating, to keep conversation state consistent for the next
            // provider call.
            let tool_result = match execute_tool_call(tool_call, &tool_context, broker).await {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("  [tool] {} — execution error: {error}", tool_call.name);
                    rc_core::ToolResult {
                        content: format!("Tool execution error: {error}"),
                        is_error: true,
                    }
                }
            };

            // Truncate tool output for context management
            let truncated_output = context_manager.truncate_tool_output_default(&tool_result.content);

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
        println!("  [tool] {tool_name} — ERROR: {}", truncate_display(display_text, 200));
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

// ---------------------------------------------------------------------------
// Slash commands
// ---------------------------------------------------------------------------

/// Handle slash commands.
///
/// Modifies `conversation` in-place for /clear and /compact! commands.
/// Returns a [`SlashCommandAction`] indicating what the caller should do next.
#[allow(clippy::too_many_lines)]
fn handle_slash_command(
    input: &str,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    context_manager: &ContextWindowManager,
    cost_tracker: &CostTracker,
) -> SlashCommandAction {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();

    match command {
        "/help" => {
            println!("Available commands:");
            println!("  /help      Show this help");
            println!("  /status    Show session and provider details");
            println!("  /compact   Show context compaction info");
            println!("  /compact!  Force context compaction now");
            println!("  /tools     List available tools");
            println!("  /sessions  List recent sessions");
            println!("  /cost      Show accumulated cost summary");
            println!("  /clear     Clear conversation (keeps system prompt)");
            println!("  /quit      Exit the interactive session");
        }
        "/status" => {
            println!("Session:  {}", config.session_id);
            println!("CWD:      {}", config.cwd.display());
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
            println!("Permission mode: {}", config.permission_mode.as_legacy_str());
            println!("Conversation entries: {}", conversation.len());
            let usage_ratio = context_manager.usage_ratio(conversation);
            println!("Context usage: {:.1}%", usage_ratio * 100.0);
            let total_cost = cost_tracker.total_cost_usd();
            if total_cost > 0.0 {
                println!("Estimated cost: ${total_cost:.6} USD");
            }
        }
        "/compact!" => {
            // Force compaction regardless of current threshold
            let before = conversation.len();
            let compacted = context_manager.compact(conversation);
            let removed = before.saturating_sub(compacted.len());
            *conversation = compacted;
            if removed > 0 {
                println!("Force-compacted: removed {removed} entries.");
            } else {
                println!("Conversation is too short to compact (needs more than 8 non-system entries).");
            }
        }
        "/compact" => {
            let ratio = context_manager.usage_ratio(conversation);
            println!("Context usage: {:.1}%", ratio * 100.0);
            println!("Available budget: {} tokens", context_manager.available_budget());
            if context_manager.needs_compaction(conversation) {
                println!("Compaction will be applied on the next turn.");
            } else {
                println!("Context is within budget — no compaction needed.");
            }
        }
        "/tools" => {
            let specs = builtin_tool_specs();
            println!("Available tools ({}):", specs.len());
            for spec in &specs {
                let perm = if spec.requires_permission { "*" } else { " " };
                println!("  {perm} {} — {}", spec.name, spec.description);
            }
            println!("  (* = requires permission)");
        }
        "/sessions" => {
            match store.list_sessions() {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No sessions found.");
                    } else {
                        for session in sessions.iter().take(10) {
                            println!(
                                "  {}  {}  {}",
                                session.session_id, session.updated_at, session.title
                            );
                        }
                    }
                }
                Err(error) => eprintln!("Error listing sessions: {error}"),
            }
        }
        "/cost" => {
            print!("{}", cost_tracker.summary());
        }
        "/clear" => {
            // Preserve the system prompt, clear everything else.
            // Note: the transcript file (JSONL) is append-only, so old entries
            // remain on disk. This only clears the in-memory conversation that
            // gets sent to the provider.
            let system_prompt = conversation
                .iter()
                .find(|e| matches!(e.role, ConversationRole::System))
                .cloned()
                .unwrap_or_else(|| ConversationEntry::system(default_system_prompt(&config.cwd)));
            conversation.clear();
            conversation.push(system_prompt);
            println!("Conversation cleared (system prompt preserved).");
            println!("Note: transcript history is still saved in the session file.");
            return SlashCommandAction::ResetScroll;
        }
        "/quit" | "/exit" => return SlashCommandAction::Quit,
        _ => {
            println!("Unknown command `{trimmed}`. Type /help for a list of commands.");
        }
    }
    SlashCommandAction::Continue
}

/// Complete a partial slash command input.
fn complete_slash_command(partial: &str) -> Vec<String> {
    let all_commands = [
        "/help", "/status", "/cost", "/compact", "/clear",
        "/sessions", "/doctor", "/quit", "/exit",
    ];
    all_commands
        .iter()
        .filter(|cmd| cmd.starts_with(partial))
        .map(|cmd| cmd.to_string())
        .collect::<Vec<_>>()
}
