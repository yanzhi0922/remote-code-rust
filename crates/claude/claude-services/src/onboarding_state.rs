//! Project onboarding state machine.
//!
//! Mirrors `cc-haha/src/projectOnboardingState.ts`. Tracks the state of first-run
//! setup flow: provider configuration, model selection, MCP discovery, and
//! initial session creation.

use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Steps in the onboarding flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    Welcome,
    ProviderSetup,
    ModelSelection,
    McpDiscovery,
    PermissionsConfig,
    RemoteSetup,
    Complete,
}

impl OnboardingStep {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::ProviderSetup => "Provider Setup",
            Self::ModelSelection => "Model Selection",
            Self::McpDiscovery => "MCP Discovery",
            Self::PermissionsConfig => "Permissions Configuration",
            Self::RemoteSetup => "Remote Setup",
            Self::Complete => "Complete",
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::ProviderSetup),
            Self::ProviderSetup => Some(Self::ModelSelection),
            Self::ModelSelection => Some(Self::McpDiscovery),
            Self::McpDiscovery => Some(Self::PermissionsConfig),
            Self::PermissionsConfig => Some(Self::RemoteSetup),
            Self::RemoteSetup => Some(Self::Complete),
            Self::Complete => None,
        }
    }
}

impl Default for OnboardingStep {
    fn default() -> Self {
        Self::Welcome
    }
}

/// Onboarding session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub completed: bool,
    pub current_step: OnboardingStep,
    pub provider_configured: bool,
    pub model_selected: bool,
    pub mcp_discovered: bool,
    pub permissions_configured: bool,
    pub remote_configured: bool,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            completed: false,
            current_step: OnboardingStep::Welcome,
            provider_configured: false,
            model_selected: false,
            mcp_discovered: false,
            permissions_configured: false,
            remote_configured: false,
        }
    }
}

impl OnboardingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&mut self) -> bool {
        if let Some(next) = self.current_step.next() {
            self.current_step = next;
            true
        } else {
            self.completed = true;
            false
        }
    }

    pub fn progress_pct(&self) -> usize {
        if self.completed {
            return 100;
        }
        match self.current_step {
            OnboardingStep::Welcome => 0,
            OnboardingStep::ProviderSetup => 15,
            OnboardingStep::ModelSelection => 30,
            OnboardingStep::McpDiscovery => 50,
            OnboardingStep::PermissionsConfig => 70,
            OnboardingStep::RemoteSetup => 85,
            OnboardingStep::Complete => 100,
        }
    }

    pub fn complete_step(&mut self) {
        match self.current_step {
            OnboardingStep::Welcome => {}
            OnboardingStep::ProviderSetup => self.provider_configured = true,
            OnboardingStep::ModelSelection => self.model_selected = true,
            OnboardingStep::McpDiscovery => self.mcp_discovered = true,
            OnboardingStep::PermissionsConfig => self.permissions_configured = true,
            OnboardingStep::RemoteSetup => self.remote_configured = true,
            OnboardingStep::Complete => self.completed = true,
        }
        self.advance();
    }
}

/// Manages onboarding state persistence.
pub struct OnboardingManager {
    state: Mutex<OnboardingState>,
}

impl OnboardingManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(OnboardingState::new()),
        }
    }

    pub fn state(&self) -> OnboardingState {
        self.state.lock().unwrap().clone()
    }

    pub fn complete_current_step(&self) -> OnboardingStep {
        let mut state = self.state.lock().unwrap();
        state.complete_step();
        state.current_step
    }

    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        *state = OnboardingState::new();
    }

    pub fn is_completed(&self) -> bool {
        self.state.lock().unwrap().completed
    }
}

impl Default for OnboardingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_welcome() {
        let state = OnboardingState::new();
        assert_eq!(state.current_step, OnboardingStep::Welcome);
        assert!(!state.completed);
    }

    #[test]
    fn progresses_through_steps() {
        let mut state = OnboardingState::new();
        assert_eq!(state.current_step, OnboardingStep::Welcome);
        // Complete Welcome (no flag set, advances to ProviderSetup)
        state.complete_step();
        assert_eq!(state.current_step, OnboardingStep::ProviderSetup);
        // Complete ProviderSetup (sets provider_configured, advances to ModelSelection)
        state.complete_step();
        assert_eq!(state.current_step, OnboardingStep::ModelSelection);
        assert!(state.provider_configured);
    }

    #[test]
    fn completes_after_all_steps() {
        let mgr = OnboardingManager::new();
        for _ in 0..7 {
            mgr.complete_current_step();
        }
        assert!(mgr.is_completed());
    }

    #[test]
    fn reset_restores_initial_state() {
        let mgr = OnboardingManager::new();
        mgr.complete_current_step();
        mgr.reset();
        assert_eq!(mgr.state().current_step, OnboardingStep::Welcome);
        assert!(!mgr.is_completed());
    }

    #[test]
    fn progress_percentage_monotonic() {
        let mut state = OnboardingState::new();
        let mut prev = 0usize;
        for _ in 0..7 {
            let pct = state.progress_pct();
            assert!(pct >= prev);
            prev = pct;
            state.complete_step();
        }
    }
}
