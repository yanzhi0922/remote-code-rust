//! Interactive TUI for Remote Code Rust.
//!
//! Uses a simple async stdin/stdout loop for maximum Windows compatibility,
//! avoiding the complexity of ratatui/crossterm raw-mode TUI.
//!
//! Supports basic Vim key bindings in the input loop:
//! - `h/j/k/l` — navigate message history (normal mode)
//! - `i` — enter insert (input) mode
//! - `Esc` — return to normal mode (type "esc" + Enter in line-buffered terminals)
//! - `G` — jump to bottom of history
//! - `gg` — jump to top of history
//! - `:q` — quit

use std::io::{self, Write};

use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, default_system_prompt};
use rc_permissions::StaticPermissionBroker;
use rc_provider::ProviderClient;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
use tokio::io::AsyncBufReadExt;

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

/// Run the interactive TUI application with a simple async input loop.
///
/// This is the main interactive mode entry point, providing:
/// - Multi-turn conversation with the provider
/// - Automatic tool execution with permission checks
/// - Context window compaction
/// - Cost tracking across turns
/// - Slash commands for session management
/// - Graceful Ctrl+C handling during input
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

    println!("Remote Code Rust — Interactive Mode");
    println!("Session:  {}", config.session_id);
    println!(
        "Provider: {} ({})",
        config.provider.name,
        config.provider.protocol.as_str()
    );
    println!(
        "Model:    {}",
        config.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!("Type /help for commands, /quit to exit");
    println!("Vim mode: Esc=normal, i=insert, j/k=scroll, G=bottom, gg=top, :q=quit");
    println!();

    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    let mut vim_mode = VimMode::Insert;
    let mut history_scroll: usize = 0; // offset from the end of conversation
    let mut pending_g = false; // for 'gg' detection

    loop {
        let prompt = match vim_mode {
            VimMode::Insert => "> ",
            VimMode::Normal => "(n) ",
        };
        print!("{prompt}");
        io::stdout().flush()?;

        // Use tokio::select! to handle Ctrl+C gracefully during input.
        // During provider calls (inside run_conversation_turn), Ctrl+C will
        // still terminate the process — this is standard CLI behavior.
        let line_opt = tokio::select! {
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => Some(line),
                    Ok(None) => None, // EOF
                    Err(e) => return Err(e.into()),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nInterrupted. Goodbye!");
                None
            }
        };

        let line = match line_opt {
            Some(line) => line,
            None => break,
        };

        let input = line.trim();

        match vim_mode {
            VimMode::Normal => {
                // Handle pending 'g' for 'gg' sequence
                if pending_g {
                    pending_g = false;
                    if input == "g" {
                        // gg — jump to top
                        history_scroll = conversation.len();
                        println!("  [top of history]");
                        continue;
                    }
                    // Not 'gg', ignore the pending g
                }

                match input {
                    "i" | "a" => {
                        vim_mode = VimMode::Insert;
                        println!("  -- INSERT --");
                    }
                    "j" => {
                        // Scroll down (toward newer messages)
                        if history_scroll > 0 {
                            history_scroll -= 1;
                            let idx = conversation.len().saturating_sub(history_scroll + 1);
                            if let Some(entry) = conversation.get(idx) {
                                print_entry(entry);
                            }
                        } else {
                            println!("  [at bottom]");
                        }
                    }
                    "k" => {
                        // Scroll up (toward older messages)
                        if history_scroll < conversation.len() {
                            history_scroll += 1;
                            let idx = conversation.len().saturating_sub(history_scroll + 1);
                            if let Some(entry) = conversation.get(idx) {
                                print_entry(entry);
                            }
                        } else {
                            println!("  [at top]");
                        }
                    }
                    "h" | "l" => {
                        // h/l: no-op in this simplified mode (left/right)
                    }
                    "G" => {
                        // Jump to bottom
                        history_scroll = 0;
                        if let Some(entry) = conversation.last() {
                            print_entry(entry);
                        }
                    }
                    "g" => {
                        // Start 'gg' sequence
                        pending_g = true;
                    }
                    ":q" | "q" => {
                        println!("Goodbye!");
                        break;
                    }
                    "" => {
                        // Empty input in normal mode, do nothing
                    }
                    _ => {
                        println!("  Unknown normal-mode key: '{input}'. Press i for insert, :q to quit.");
                    }
                }
                continue;
            }
            VimMode::Insert => {
                // In line-buffered mode, the terminal doesn't send raw escape
                // sequences. Users can switch to normal mode by typing "esc",
                // "ESC", or the literal escape character followed by Enter.
                if input.eq_ignore_ascii_case("esc")
                    || input == "\u{1b}"
                    || input == "^["
                {
                    vim_mode = VimMode::Normal;
                    println!("  -- NORMAL --");
                    continue;
                }
            }
        }

        if input.is_empty() {
            continue;
        }

        // Handle slash commands
        if input.starts_with('/') {
            match handle_slash_command(
                input,
                &config,
                store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
            ) {
                SlashCommandAction::Quit => break,
                SlashCommandAction::ResetScroll => history_scroll = 0,
                SlashCommandAction::Continue => {}
            }
            continue;
        }

        // Execute conversation turn
        if let Err(error) = run_conversation_turn(
            &provider,
            &config,
            store,
            &mut conversation,
            &context_manager,
            &broker,
            &cost_tracker,
            input,
        )
        .await
        {
            eprintln!("Error: {error}");
            eprintln!("  (your message was saved; the conversation state may be inconsistent)");
        }
    }

    // Print cost summary on exit
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

/// Print a conversation entry for Vim-mode history navigation.
fn print_entry(entry: &ConversationEntry) {
    let role = match entry.role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    };
    let text = entry.history_text();
    let preview: String = text.chars().take(200).collect();
    println!("  [{role}] {preview}");
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
