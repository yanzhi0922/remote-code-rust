//! Roo CLI — command-line interface for Roo Code Rust.
//!
//! Supports sending messages to AI providers and streaming responses.
//! Implements a full tool-call execution loop: user input → API call →
//! tool execution → feedback → loop until text-only response.

use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use futures::StreamExt;

use roo_app::{App, AppConfig};
use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_provider_anthropic::{AnthropicConfig, AnthropicHandler, AnthropicVertexConfig, AnthropicVertexHandler};
use roo_provider_openai::{OpenAiConfig, OpenAiHandler};
use roo_provider_openai_native::OpenAiNativeConfig;
use roo_provider_openai_native::OpenAiNativeHandler;
use roo_provider_openrouter::{OpenRouterConfig, OpenRouterHandler};
use roo_provider_deepseek::{DeepSeekConfig, DeepSeekHandler};
use roo_provider_google::{GoogleConfig, GoogleHandler};
use roo_provider_xai::{XaiConfig, XaiHandler};
use roo_provider_mistral::{MistralConfig, MistralHandler};
use roo_provider_fireworks::{FireworksConfig, FireworksHandler};
use roo_provider_ollama::{OllamaConfig, OllamaHandler};
use roo_provider_lmstudio::{LmStudioConfig, LmStudioHandler};
use roo_provider_litellm::{LiteLlmConfig, LiteLlmHandler};
use roo_provider_qwen::{QwenConfig, QwenHandler};
use roo_provider_minimax::{MiniMaxConfig, MiniMaxHandler};
use roo_provider_poe::{PoeConfig, PoeHandler};
use roo_provider_requesty::{RequestyConfig, RequestyHandler};
use roo_provider_unbound::{UnboundConfig, UnboundHandler};
use roo_provider_vercel::{VercelConfig, VercelHandler};
use roo_provider_roo::{RooConfig, RooHandler};
use roo_provider_sambanova::{SambaNovaConfig, SambaNovaHandler};
use roo_provider_baseten::{BasetenConfig, BasetenHandler};
use roo_provider_moonshot::{MoonshotConfig, MoonshotHandler};
use roo_provider_zai::{ZaiConfig, ZaiHandler};
use roo_provider_aws::{AwsBedrockConfig, AwsBedrockHandler};
use roo_task::tool_dispatcher::{
    ToolContext, ToolDispatcher, ToolExecutionResult,
    default_dispatcher_with_terminal,
};
use roo_terminal::TerminalRegistry;
use roo_tools::definition::{NativeToolsOptions, get_native_tools};
use roo_types::api::{ApiMessage, ApiStreamChunk, ContentBlock, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// CLI argument definitions
// ---------------------------------------------------------------------------

/// Roo CLI — interact with AI providers from the command line.
#[derive(Debug, Parser)]
#[command(name = "roo", version, about = "Roo Code CLI")]
struct Cli {
    /// AI provider to use (e.g. "anthropic").
    #[arg(long, default_value = "anthropic")]
    provider: String,

    /// Base URL for the provider API.
    #[arg(long)]
    base_url: Option<String>,

    /// API key for authentication.
    #[arg(long)]
    api_key: Option<String>,

    /// Model ID to use (e.g. "claude-sonnet-4-20250514").
    #[arg(long)]
    model: Option<String>,

    /// Temperature for generation (0.0 – 1.0).
    #[arg(long)]
    temperature: Option<f64>,

    /// Enable extended / thinking mode (Anthropic only).
    #[arg(long)]
    thinking: bool,

    /// Max thinking tokens when extended thinking is enabled.
    #[arg(long)]
    max_thinking_tokens: Option<u64>,

    /// Request timeout in milliseconds.
    #[arg(long)]
    timeout: Option<u64>,

    /// System prompt override. If omitted, a default prompt is generated.
    #[arg(long)]
    system_prompt: Option<String>,

    /// Single message to send (non-interactive mode).
    #[arg(short, long)]
    message: Option<String>,

    /// Launch interactive REPL mode.
    #[arg(short, long)]
    interactive: bool,

    /// Path to a JSON configuration file.
    #[arg(short, long)]
    config: Option<String>,

    /// Working directory for tool execution (default: current directory).
    #[arg(long, global = true)]
    working_dir: Option<String>,
}

/// Configuration loaded from a JSON file.
#[derive(Debug, serde::Deserialize)]
struct ConfigFile {
    provider: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    thinking: Option<bool>,
    max_thinking_tokens: Option<u64>,
    timeout: Option<u64>,
    system_prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// Collected tool call (accumulated from stream chunks)
// ---------------------------------------------------------------------------

/// A tool call accumulated from streaming chunks.
#[derive(Debug, Clone)]
struct CollectedToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// A thinking/reasoning block accumulated from streaming chunks.
#[derive(Debug, Clone)]
struct CollectedThinkingBlock {
    text: String,
    signature: Option<String>,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Install a default tracing subscriber so provider internals can log.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    // Switch working directory if --working-dir is specified.
    if let Some(ref dir) = cli.working_dir {
        std::env::set_current_dir(dir)
            .with_context(|| format!("Failed to change working directory to: {dir}"))?;
    }

    // ── Initialize through the App layer ────────────────────────────────
    // This gives CLI users access to MCP tools, Skills, RooIgnore, etc.
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let app_config = AppConfig {
        cwd: cwd.clone(),
        global_storage_path: home_roo_dir(),
        mode: "code".to_string(),
        ..Default::default()
    };

    let mut app = App::new(app_config);
    app.initialize().await
        .context("Failed to initialize App layer")?;
    tracing::info!("App layer initialized for CLI");

    // Load optional config file and merge with CLI flags (CLI takes priority).
    let config = load_config(&cli)?;

    // Build the provider handler.
    let provider_name = config.provider.as_deref().unwrap_or("anthropic");
    let handler = build_handler(provider_name, &config)
        .context("Failed to create provider handler")?;

    // Build the system prompt using the App layer (includes MCP/Skills awareness).
    let system_prompt = config.system_prompt.unwrap_or_else(|| app.build_system_prompt());

    // Build tool definitions (JSON values for the API).
    let tool_defs = get_native_tools(NativeToolsOptions::default());
    let tools_json: Vec<serde_json::Value> = tool_defs
        .iter()
        .map(|td| serde_json::to_value(td).unwrap_or_default())
        .collect();

    // Use the terminal registry from the App layer.
    let registry = app.terminal_registry()
        .cloned()
        .unwrap_or_else(|| Arc::new(TerminalRegistry::new()));
    let working_dir = PathBuf::from(app.cwd());
    let output_dir = std::env::temp_dir().join("roo-cli-output");
    std::fs::create_dir_all(&output_dir).ok();

    let dispatcher = default_dispatcher_with_terminal(registry, output_dir, "code");

    let result = if cli.interactive {
        run_interactive(&*handler, &system_prompt, &tools_json, &dispatcher, &working_dir).await
    } else if let Some(msg) = &cli.message {
        run_single(
            &*handler,
            &system_prompt,
            &tools_json,
            &dispatcher,
            &working_dir,
            msg,
        )
        .await
    } else {
        anyhow::bail!(
            "Provide --message <text> for single-shot mode or --interactive for REPL mode."
        )
    };

    // Clean up App resources.
    let _ = app.dispose().await;
    result
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

/// Merge CLI flags with an optional JSON config file. CLI flags take priority.
fn load_config(cli: &Cli) -> Result<ConfigFile> {
    let mut cfg = if let Some(path) = &cli.config {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {path}"))?;
        serde_json::from_str::<ConfigFile>(&raw)
            .with_context(|| format!("Failed to parse config file: {path}"))?
    } else {
        ConfigFile {
            provider: None,
            base_url: None,
            api_key: None,
            model: None,
            temperature: None,
            thinking: None,
            max_thinking_tokens: None,
            timeout: None,
            system_prompt: None,
        }
    };

    // CLI flags override config file values.
    if cli.provider != "anthropic" || cfg.provider.is_none() {
        cfg.provider = Some(cli.provider.clone());
    }
    if cli.base_url.is_some() {
        cfg.base_url = cli.base_url.clone();
    }
    if cli.api_key.is_some() {
        cfg.api_key = cli.api_key.clone();
    }
    if cli.model.is_some() {
        cfg.model = cli.model.clone();
    }
    if cli.temperature.is_some() {
        cfg.temperature = cli.temperature;
    }
    if cli.thinking {
        cfg.thinking = Some(true);
    }
    if cli.max_thinking_tokens.is_some() {
        cfg.max_thinking_tokens = cli.max_thinking_tokens;
    }
    if cli.timeout.is_some() {
        cfg.timeout = cli.timeout;
    }
    if cli.system_prompt.is_some() {
        cfg.system_prompt = cli.system_prompt.clone();
    }

    Ok(cfg)
}

// ---------------------------------------------------------------------------
// Provider construction
// ---------------------------------------------------------------------------

/// Build a boxed Provider based on the provider name.
fn build_handler(
    provider_name: &str,
    config: &ConfigFile,
) -> Result<Box<dyn Provider>> {
    match provider_name {
        // ── Anthropic ──────────────────────────────────────────────────
        "anthropic" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for anthropic"))?;
            let cfg = AnthropicConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| AnthropicConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                use_extended_thinking: config.thinking,
                max_thinking_tokens: config.max_thinking_tokens,
                request_timeout: config.timeout,
            };
            Ok(Box::new(AnthropicHandler::new(cfg)
                .context("Failed to create Anthropic handler")?))
        }

        // ── Anthropic Vertex ───────────────────────────────────────────
        "vertex" => {
            let project_id = config.base_url.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--base-url is required for vertex (use project_id)"))?
                .to_string();
            let access_token = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for vertex (use access_token)"))?
                .to_string();
            let cfg = AnthropicVertexConfig {
                project_id,
                region: "us-east5".to_string(),
                access_token,
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
                enable_1m_context: false,
                use_extended_thinking: config.thinking,
                max_thinking_tokens: config.max_thinking_tokens,
            };
            Ok(Box::new(AnthropicVertexHandler::new(cfg)
                .context("Failed to create Anthropic Vertex handler")?))
        }

        // ── AWS Bedrock ────────────────────────────────────────────────
        "aws" | "bedrock" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for bedrock (format: access_key:secret_key)"))?;
            let parts: Vec<&str> = api_key.splitn(2, ':').collect();
            let (access_key, secret_key) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                anyhow::bail!("--api-key for bedrock must be in format access_key:secret_key");
            };
            let cfg = AwsBedrockConfig {
                access_key,
                secret_key,
                session_token: None,
                region: config.base_url.clone()
                    .unwrap_or_else(|| AwsBedrockConfig::DEFAULT_REGION.to_string()),
                model_id: config.model.clone(),
                use_cross_region_inference: false,
                endpoint_url: None,
                request_timeout: config.timeout,
                temperature: None,
            };
            Ok(Box::new(AwsBedrockHandler::new(cfg)
                .context("Failed to create Bedrock handler")?))
        }

        // ── OpenAI ─────────────────────────────────────────────────────
        "openai" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for openai"))?;
            let cfg = OpenAiConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| OpenAiConfig::DEFAULT_BASE_URL.to_string()),
                org_id: None,
                model_id: config.model.clone(),
                temperature: config.temperature,
                reasoning_effort: None,
                request_timeout: config.timeout,
            };
            Ok(Box::new(OpenAiHandler::new(cfg)
                .context("Failed to create OpenAI handler")?))
        }

        // ── OpenAI Native (Responses API) ──────────────────────────────
        "openai-native" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for openai-native"))?;
            let cfg = OpenAiNativeConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone(),
                model_id: config.model.clone(),
                temperature: config.temperature,
                reasoning_effort: None,
                request_timeout: config.timeout,
                service_tier: None,
                enable_reasoning_summary: true,
            };
            Ok(Box::new(OpenAiNativeHandler::new(cfg)
                .context("Failed to create OpenAI Native handler")?))
        }

        // ── OpenRouter ─────────────────────────────────────────────────
        "openrouter" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for openrouter"))?;
            let cfg = OpenRouterConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| OpenRouterConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(OpenRouterHandler::new(cfg)
                .context("Failed to create OpenRouter handler")?))
        }

        // ── DeepSeek ───────────────────────────────────────────────────
        "deepseek" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for deepseek"))?;
            let cfg = DeepSeekConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| DeepSeekConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(DeepSeekHandler::new(cfg)
                .context("Failed to create DeepSeek handler")?))
        }

        // ── Google Gemini ──────────────────────────────────────────────
        "google" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for google"))?;
            let cfg = GoogleConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| GoogleConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(GoogleHandler::new(cfg)
                .context("Failed to create Google handler")?))
        }

        // ── xAI ────────────────────────────────────────────────────────
        "xai" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for xai"))?;
            let cfg = XaiConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| XaiConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(XaiHandler::new(cfg)
                .context("Failed to create xAI handler")?))
        }

        // ── Mistral ────────────────────────────────────────────────────
        "mistral" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for mistral"))?;
            let cfg = MistralConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| MistralConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(MistralHandler::new(cfg)
                .context("Failed to create Mistral handler")?))
        }

        // ── Fireworks ──────────────────────────────────────────────────
        "fireworks" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for fireworks"))?;
            let cfg = FireworksConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| FireworksConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(FireworksHandler::new(cfg)
                .context("Failed to create Fireworks handler")?))
        }

        // ── Ollama (no API key needed) ─────────────────────────────────
        "ollama" => {
            let cfg = OllamaConfig {
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| OllamaConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
                api_options: None,
            };
            Ok(Box::new(OllamaHandler::new(cfg)
                .context("Failed to create Ollama handler")?))
        }

        // ── LM Studio (no API key needed) ──────────────────────────────
        "lmstudio" => {
            let cfg = LmStudioConfig {
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| LmStudioConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
                speculative_decoding_enabled: false,
                draft_model_id: None,
            };
            Ok(Box::new(LmStudioHandler::new(cfg)
                .context("Failed to create LM Studio handler")?))
        }

        // ── LiteLLM ────────────────────────────────────────────────────
        "litellm" => {
            let api_key = config.api_key.clone()
                .unwrap_or_else(|| "dummy-key".to_string());
            let cfg = LiteLlmConfig {
                api_key,
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| LiteLlmConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                use_prompt_cache: false,
                request_timeout: config.timeout,
            };
            Ok(Box::new(LiteLlmHandler::new(cfg)
                .context("Failed to create LiteLLM handler")?))
        }

        // ── Qwen ───────────────────────────────────────────────────────
        "qwen" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for qwen"))?;
            let cfg = QwenConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| QwenConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(QwenHandler::new(cfg)
                .context("Failed to create Qwen handler")?))
        }

        // ── MiniMax ────────────────────────────────────────────────────
        "minimax" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for minimax"))?;
            let cfg = MiniMaxConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| MiniMaxConfig::DEFAULT_BASE_URL.to_string()),
                group_id: None,
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(MiniMaxHandler::new(cfg)
                .context("Failed to create MiniMax handler")?))
        }

        // ── Poe ────────────────────────────────────────────────────────
        "poe" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for poe"))?;
            let cfg = PoeConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone(),
                model_id: config.model.clone(),
                temperature: config.temperature,
                max_thinking_tokens: config.max_thinking_tokens,
                reasoning_effort: None,
                request_timeout: config.timeout,
            };
            Ok(Box::new(PoeHandler::new(cfg)
                .context("Failed to create Poe handler")?))
        }

        // ── Requesty ───────────────────────────────────────────────────
        "requesty" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for requesty"))?;
            let cfg = RequestyConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| RequestyConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(RequestyHandler::new(cfg)
                .context("Failed to create Requesty handler")?))
        }

        // ── Unbound ────────────────────────────────────────────────────
        "unbound" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for unbound"))?;
            let cfg = UnboundConfig {
                api_key: api_key.to_string(),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(UnboundHandler::new(cfg)
                .context("Failed to create Unbound handler")?))
        }

        // ── Vercel AI Gateway ──────────────────────────────────────────
        "vercel" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for vercel"))?;
            let cfg = VercelConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone(),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(VercelHandler::new(cfg)
                .context("Failed to create Vercel handler")?))
        }

        // ── Roo Code Cloud ─────────────────────────────────────────────
        "roo" => {
            let cfg = RooConfig {
                api_key: config.api_key.clone(),
                base_url: config.base_url.clone()
                    .or_else(|| Some(RooConfig::DEFAULT_BASE_URL.to_string())),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(RooHandler::new(cfg)
                .context("Failed to create Roo handler")?))
        }

        // ── SambaNova ──────────────────────────────────────────────────
        "sambanova" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for sambanova"))?;
            let cfg = SambaNovaConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| SambaNovaConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(SambaNovaHandler::new(cfg)
                .context("Failed to create SambaNova handler")?))
        }

        // ── Baseten ────────────────────────────────────────────────────
        "baseten" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for baseten"))?;
            let cfg = BasetenConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| BasetenConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(BasetenHandler::new(cfg)
                .context("Failed to create Baseten handler")?))
        }

        // ── Moonshot ───────────────────────────────────────────────────
        "moonshot" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for moonshot"))?;
            let cfg = MoonshotConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| MoonshotConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(MoonshotHandler::new(cfg)
                .context("Failed to create Moonshot handler")?))
        }

        // ── ZAI ────────────────────────────────────────────────────────
        "zai" => {
            let api_key = config.api_key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--api-key is required for zai"))?;
            let cfg = ZaiConfig {
                api_key: api_key.to_string(),
                base_url: config.base_url.clone()
                    .unwrap_or_else(|| ZaiConfig::DEFAULT_BASE_URL.to_string()),
                model_id: config.model.clone(),
                temperature: config.temperature,
                request_timeout: config.timeout,
            };
            Ok(Box::new(ZaiHandler::new(cfg)
                .context("Failed to create ZAI handler")?))
        }

        other => anyhow::bail!(
            "Unsupported provider: '{other}'. Supported providers: anthropic, vertex, aws/bedrock, \
             openai, openai-native, openrouter, deepseek, google, xai, mistral, fireworks, \
             ollama, lmstudio, litellm, qwen, minimax, poe, requesty, unbound, vercel, \
             roo, sambanova, baseten, moonshot, zai"
        ),
    }
}

// ---------------------------------------------------------------------------
// Single-shot mode (with tool execution loop)
// ---------------------------------------------------------------------------

/// Send one message and run the full tool-call loop.
async fn run_single(
    handler: &dyn Provider,
    system_prompt: &str,
    tools_json: &[serde_json::Value],
    dispatcher: &ToolDispatcher,
    working_dir: &PathBuf,
    message: &str,
) -> Result<()> {
    let mut messages = vec![ApiMessage {
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: message.to_string(),
        }],
        reasoning: None,
        ts: None,
        truncation_parent: None,
        is_truncation_marker: None,
        truncation_id: None,
        condense_parent: None,
        is_summary: None,
        condense_id: None,
            reasoning_details: None,
    }];

    loop {
        let metadata = CreateMessageMetadata::default();

        let stream = handler
            .create_message(system_prompt, messages.clone(), Some(tools_json.to_vec()), metadata)
            .await
            .context("Failed to create message")?;

        let (assistant_text, tool_calls, thinking_blocks) = collect_stream(stream).await?;

        // P1-2 extended: Filter out tool calls with empty names before building messages
        let tool_calls: Vec<CollectedToolCall> = tool_calls
            .into_iter()
            .filter(|tc| !tc.name.is_empty())
            .collect();

        // Build assistant message content blocks.
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        // P1-1: Include thinking blocks in assistant message
        for tb in &thinking_blocks {
            assistant_content.push(ContentBlock::Thinking {
                thinking: tb.text.clone(),
                signature: tb.signature.clone().unwrap_or_default(),
            });
        }
        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: assistant_text.clone(),
            });
        }
        for tc in &tool_calls {
            // P0-1 fix: Default to empty object {} instead of null
            let input: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            assistant_content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input,
            });
        }

        messages.push(ApiMessage {
            role: MessageRole::Assistant,
            content: assistant_content,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });

        // If no tool calls, we're done.
        if tool_calls.is_empty() {
            println!();
            return Ok(());
        }

        // Execute tool calls and collect results.
        let tool_results = execute_tool_calls(tool_calls, dispatcher, working_dir).await;

        messages.push(ApiMessage {
            role: MessageRole::User,
            content: tool_results,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });

        // Continue loop — let the model process tool results.
    }
}

// ---------------------------------------------------------------------------
// Interactive REPL mode (with tool execution loop)
// ---------------------------------------------------------------------------

/// Interactive read-eval-print loop with full tool-call support.
async fn run_interactive(
    handler: &dyn Provider,
    system_prompt: &str,
    tools_json: &[serde_json::Value],
    dispatcher: &ToolDispatcher,
    working_dir: &PathBuf,
) -> Result<()> {
    println!("Roo CLI — interactive mode (type :quit or Ctrl-C to exit)\n");

    let mut conversation: Vec<ApiMessage> = Vec::new();

    loop {
        // Read user input.
        let input = read_line("you> ")?;
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" || trimmed == ":q" {
            println!("Bye!");
            return Ok(());
        }
        if trimmed == ":clear" {
            conversation.clear();
            println!("[Conversation cleared]\n");
            continue;
        }

        // Append user message.
        conversation.push(ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: trimmed }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        });

        // Tool-call loop: keep calling the API until we get a text-only response.
        loop {
            let metadata = CreateMessageMetadata::default();

            let stream = handler
                .create_message(
                    system_prompt,
                    conversation.clone(),
                    Some(tools_json.to_vec()),
                    metadata,
                )
                .await
                .context("Failed to create message")?;

            print!("ai> ");
            io::stdout().flush().ok();

            let (assistant_text, tool_calls, thinking_blocks) = collect_stream(stream).await?;

            // P1-2 extended: Filter out tool calls with empty names before building messages
            let tool_calls: Vec<CollectedToolCall> = tool_calls
                .into_iter()
                .filter(|tc| !tc.name.is_empty())
                .collect();

            // Build assistant message content blocks.
            let mut assistant_content: Vec<ContentBlock> = Vec::new();
            // P1-1: Include thinking blocks in assistant message
            for tb in &thinking_blocks {
                assistant_content.push(ContentBlock::Thinking {
                    thinking: tb.text.clone(),
                    signature: tb.signature.clone().unwrap_or_default(),
                });
            }
            if !assistant_text.is_empty() {
                assistant_content.push(ContentBlock::Text {
                    text: assistant_text.clone(),
                });
            }
            for tc in &tool_calls {
                // P0-1 fix: Default to empty object {} instead of null
                let input: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
                assistant_content.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input,
                });
            }

            conversation.push(ApiMessage {
                role: MessageRole::Assistant,
                content: assistant_content,
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            });

            // If no tool calls, we're done — wait for next user input.
            if tool_calls.is_empty() {
                println!();
                break;
            }

            // Execute tool calls and collect results.
            let tool_results = execute_tool_calls(tool_calls, dispatcher, working_dir).await;

            conversation.push(ApiMessage {
                role: MessageRole::User,
                content: tool_results,
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            });

            // Continue loop — let the model process tool results.
        }
    }
}

// ---------------------------------------------------------------------------
// Stream collection — accumulate text + tool calls from API stream
// ---------------------------------------------------------------------------

/// Collect all chunks from an API stream, printing text in real-time,
/// and return the accumulated text and tool calls.
async fn collect_stream(
    mut stream: roo_provider::handler::ApiStream,
) -> Result<(String, Vec<CollectedToolCall>, Vec<CollectedThinkingBlock>)> {
    let mut collected_text = String::new();
    let mut tool_calls: Vec<CollectedToolCall> = Vec::new();
    // Map from tool call id → index in tool_calls vec (for delta accumulation).
    let mut tool_call_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // P1-1: Thinking/reasoning state
    let mut thinking_blocks: Vec<CollectedThinkingBlock> = Vec::new();
    let mut current_thinking_text: String = String::new();
    let mut current_thinking_signature: Option<String> = None;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => process_chunk(
                &chunk,
                &mut collected_text,
                &mut tool_calls,
                &mut tool_call_index,
                &mut thinking_blocks,
                &mut current_thinking_text,
                &mut current_thinking_signature,
            ),
            Err(e) => {
                eprintln!("\n[stream error] {e}");
            }
        }
    }

    // P1-1: Flush any unfinished thinking block
    if !current_thinking_text.is_empty() {
        thinking_blocks.push(CollectedThinkingBlock {
            text: current_thinking_text,
            signature: current_thinking_signature,
        });
    }

    // Deduplicate tool call IDs — some providers (e.g. MiniMax) may return
    // multiple tool calls with the same ID. Append a suffix to duplicates so
    // that subsequent API calls don't fail with "duplicate tool_call id".
    {
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tc in &mut tool_calls {
            if seen_ids.contains(&tc.id) {
                let original_id = tc.id.clone();
                let mut suffix = 2u32;
                loop {
                    let new_id = format!("{}_dedup{}", original_id, suffix);
                    if !seen_ids.contains(&new_id) {
                        tc.id = new_id;
                        break;
                    }
                    suffix += 1;
                }
                eprintln!("\n\x1b[33m[warn] deduplicated tool call id: {} -> {}\x1b[0m", original_id, tc.id);
            }
            seen_ids.insert(tc.id.clone());
        }
    }

    Ok((collected_text, tool_calls, thinking_blocks))
}

/// Process a single stream chunk, printing text and accumulating tool calls.
fn process_chunk(
    chunk: &ApiStreamChunk,
    collected_text: &mut String,
    tool_calls: &mut Vec<CollectedToolCall>,
    tool_call_index: &mut std::collections::HashMap<String, usize>,
    thinking_blocks: &mut Vec<CollectedThinkingBlock>,
    current_thinking_text: &mut String,
    _current_thinking_signature: &mut Option<String>,
) {
    match chunk {
        ApiStreamChunk::Text { text } => {
            print!("{text}");
            io::stdout().flush().ok();
            collected_text.push_str(text);
        }
        ApiStreamChunk::Reasoning { text, .. } => {
            print!("\x1b[90m{text}\x1b[0m");
            io::stdout().flush().ok();
            // P1-1: Accumulate thinking/reasoning text for saving to assistant message
            current_thinking_text.push_str(text);
        }
        ApiStreamChunk::ThinkingComplete { signature } => {
            print!(
                "\x1b[90m[thinking complete: sig={}]\x1b[0m",
                &signature[..signature.len().min(16)]
            );
            // P1-1: Finalize the thinking block
            let text = std::mem::take(current_thinking_text);
            if !text.is_empty() {
                thinking_blocks.push(CollectedThinkingBlock {
                    text,
                    signature: Some(signature.clone()),
                });
            }
        }
        ApiStreamChunk::Usage {
            input_tokens,
            output_tokens,
            cache_write_tokens,
            cache_read_tokens,
            reasoning_tokens,
            total_cost,
        } => {
            eprintln!(
                "\n[usage] in={input_tokens} out={output_tokens} cache_w={:?} cache_r={:?} reasoning={:?} cost={:?}",
                cache_write_tokens, cache_read_tokens, reasoning_tokens, total_cost,
            );
        }
        ApiStreamChunk::ToolCallStart { id, name } => {
            print!("\n\x1b[36m[tool: {name}]\x1b[0m ");
            io::stdout().flush().ok();
            let idx = tool_calls.len();
            tool_calls.push(CollectedToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments: String::new(),
            });
            tool_call_index.insert(id.clone(), idx);
        }
        ApiStreamChunk::ToolCallDelta { id, delta } => {
            if let Some(&idx) = tool_call_index.get(id) {
                tool_calls[idx].arguments.push_str(delta);
            }
        }
        ApiStreamChunk::ToolCallEnd { .. } => {
            // Tool call complete — nothing extra to print.
        }
        ApiStreamChunk::ToolCall {
            id,
            name,
            arguments,
        } => {
            // P0-2 fix: If a ToolCallPartial with the same ID was already collected
            // (e.g. from Anthropic handler emitting both partial and complete events),
            // replace it with the complete data instead of creating a duplicate.
            if let Some(&existing_idx) = tool_call_index.get(id) {
                tool_calls[existing_idx] = CollectedToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                };
            } else {
                // New tool call — not seen before as a partial
                print!("\n\x1b[36m[tool: {name}]\x1b[0m ");
                io::stdout().flush().ok();
                let idx = tool_calls.len();
                tool_calls.push(CollectedToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
                tool_call_index.insert(id.clone(), idx);
            }
        }
        ApiStreamChunk::ToolCallPartial {
            index,
            id,
            name,
            arguments,
        } => {
            // Partial tool call from OpenAI-compatible providers.
            let idx = *index as usize;
            if idx >= tool_calls.len() {
                tool_calls.resize(idx + 1, CollectedToolCall {
                    id: String::new(),
                    name: String::new(),
                    arguments: String::new(),
                });
            }
            if let Some(partial_id) = id {
                tool_call_index.insert(partial_id.clone(), idx);
                tool_calls[idx].id = partial_id.clone();
            }
            if let Some(partial_name) = name {
                if tool_calls[idx].name.is_empty() {
                    print!("\n\x1b[36m[tool: {partial_name}]\x1b[0m ");
                    io::stdout().flush().ok();
                }
                tool_calls[idx].name = partial_name.clone();
            }
            if let Some(partial_args) = arguments {
                tool_calls[idx].arguments.push_str(partial_args);
            }
        }
        ApiStreamChunk::Grounding { sources } => {
            eprintln!("\n[grounding: {} sources]", sources.len());
        }
        ApiStreamChunk::Error { error, message } => {
            eprintln!("\n\x1b[31m[error] {error}: {message}\x1b[0m");
        }
    }
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

/// Execute a list of tool calls and return content blocks for the results.
async fn execute_tool_calls(
    tool_calls: Vec<CollectedToolCall>,
    dispatcher: &ToolDispatcher,
    working_dir: &PathBuf,
) -> Vec<ContentBlock> {
    let mut results: Vec<ContentBlock> = Vec::new();

    for tc in &tool_calls {
        // P1-2: Skip tool calls with empty names
        if tc.name.is_empty() {
            eprintln!("\n\x1b[33m[warn] skipping tool call with empty name (id={})\x1b[0m", tc.id);
            results.push(ContentBlock::ToolResult {
                tool_use_id: tc.id.clone(),
                content: vec![ToolResultContent::Text {
                    text: "Error: tool name is empty".to_string(),
                }],
                is_error: Some(true),
            });
            continue;
        }

        // P0-1 fix: Default to empty object {} instead of null
        let params: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));

        // Display tool invocation.
        let args_preview = serde_json::to_string_pretty(&params)
            .unwrap_or_else(|_| tc.arguments.clone());
        println!(
            "\n\x1b[33m[executing] {}({})\x1b[0m",
            tc.name,
            truncate_str(&args_preview, 200)
        );

        let context = ToolContext::new(working_dir, "cli-session");

        let result = dispatcher.dispatch(&tc.name, params, &context).await;

        let (output_text, is_error) = match result {
            ToolExecutionResult {
                text,
                is_error: err,
                ..
            } => {
                let preview = truncate_str(&text, 300);
                if err {
                    eprintln!("\x1b[31m[result] Error: {}\x1b[0m", preview);
                } else {
                    println!("\x1b[32m[result] {}\x1b[0m", preview);
                }
                (text, err)
            }
        };

        results.push(ContentBlock::ToolResult {
            tool_use_id: tc.id.clone(),
            content: vec![ToolResultContent::Text {
                text: if is_error {
                    format!("Error: {}", output_text)
                } else {
                    output_text
                },
            }],
            is_error: if is_error { Some(true) } else { None },
        });
    }

    results
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Read a line from stdin with a prompt.
fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().context("Failed to flush stdout")?;
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .context("Failed to read from stdin")?;
    Ok(buf)
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find the nearest valid UTF-8 char boundary at or before max_len
        let boundary = s.floor_char_boundary(max_len);
        let mut truncated = s[..boundary].to_string();
        truncated.push_str("...");
        truncated
    }
}

/// Best-effort home directory.
fn home_dir() -> String {
    if cfg!(windows) {
        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".into())
    }
}

/// Best-effort global .roo directory path for storage.
fn home_roo_dir() -> String {
    let home = home_dir();
    let mut path = std::path::PathBuf::from(&home);
    path.push(".roo");
    path.to_string_lossy().to_string()
}
