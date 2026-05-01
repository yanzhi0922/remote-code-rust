//! VS Code integration type definitions.
//!
//! Derived from `packages/types/src/vscode.ts`.
//! These types are primarily used in VS Code extension mode;
//! in CLI mode they exist as type stubs for API compatibility.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Code actions
// ---------------------------------------------------------------------------

/// Identifier for a code editor context-menu action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodeActionId {
    ExplainCode,
    FixCode,
    ImproveCode,
    AddToContext,
    NewTask,
}

/// Short name used in prompt templates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeActionName {
    Explain,
    Fix,
    Improve,
    AddToContext,
    NewTask,
}

// ---------------------------------------------------------------------------
// Terminal actions
// ---------------------------------------------------------------------------

/// Identifier for a terminal context-menu action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalActionId {
    TerminalAddToContext,
    TerminalFixCommand,
    TerminalExplainCommand,
}

/// Short name for terminal actions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalActionName {
    AddToContext,
    Fix,
    Explain,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Well-known command identifiers exposed by the extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandId {
    ActivationCompleted,
    PlusButtonClicked,
    HistoryButtonClicked,
    MarketplaceButtonClicked,
    PopoutButtonClicked,
    CloudButtonClicked,
    SettingsButtonClicked,
    OpenInNewTab,
    NewTask,
    SetCustomStoragePath,
    ImportSettings,
    FocusInput,
    AcceptInput,
    FocusPanel,
    ToggleAutoApprove,
}

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

/// Supported UI languages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "ca")]
    Ca,
    #[serde(rename = "de")]
    De,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "es")]
    Es,
    #[serde(rename = "fr")]
    Fr,
    #[serde(rename = "hi")]
    Hi,
    #[serde(rename = "id")]
    Id,
    #[serde(rename = "it")]
    It,
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "ko")]
    Ko,
    #[serde(rename = "nl")]
    Nl,
    #[serde(rename = "pl")]
    Pl,
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "ru")]
    Ru,
    #[serde(rename = "tr")]
    Tr,
    #[serde(rename = "vi")]
    Vi,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
}

/// VS Code Language Model chat selector components.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsCodeLmModelSelector {
    pub vendor: Option<String>,
    pub family: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
}
