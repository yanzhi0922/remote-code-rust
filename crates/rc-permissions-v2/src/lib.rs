//! # rc-permissions-v2 — Advanced Permission System V2
//!
//! Full permission system matching Claude Code's `utils/permissions/` (25+ files).
//!
//! ## Features
//! - **7 Permission Modes**: default, plan, acceptEdits, bypassPermissions, dontAsk, auto, bubble
//! - **YOLO Classifier**: Auto-approves safe read operations and known-safe bash commands
//! - **Bash Classifier**: Categorizes bash commands by safety level (Safe/Development/Unknown/Dangerous/Critical)
//! - **Dangerous Pattern Detection**: Detects rm -rf /, sudo, curl|sh, force-push, etc.
//! - **Auto Mode**: Classifier-based automatic approval with state tracking
//! - **Shadowed Rule Detection**: Identifies rules that will never be evaluated
//! - **Bypass Killswitch**: Remote safety mechanism to disable bypass permissions
//! - **Filesystem Checks**: Validates file operations are within allowed scope
//! - **Path Validation**: Detects path traversal attacks and null bytes
//! - **Shell Matching**: Glob-style matching for bash command rules
//! - **Denial Tracking**: Tracks repeated denials to prevent prompt spam
//! - **Permission Explainer**: Human-readable explanations for decisions
//! - **Multiple Handlers**: Interactive, Coordinator, and SwarmWorker strategies
//!
//! ## Example
//! ```ignore
//! use rc_permissions_v2::classifier::YoloClassifier;
//! use rc_permissions_v2::dangerous_patterns::is_critically_dangerous;
//!
//! assert!(is_critically_dangerous("rm -rf /"));
//! assert!(!is_critically_dangerous("git status"));
//! ```

pub mod auto_mode;
pub mod bypass_killswitch;
pub mod classifier;
pub mod dangerous_patterns;
pub mod decision;
pub mod denial_tracking;
pub mod explainer;
pub mod filesystem;
pub mod handler;
pub mod loader;
pub mod mode;
pub mod path_validation;
pub mod rule;
pub mod setup;
pub mod shadowed_detection;
pub mod shell_matching;

pub use auto_mode::{AutoModeManager, AutoModeState};
pub use bypass_killswitch::BypassKillswitchManager;
pub use classifier::{BashClassifier, BashCommandCategory, ClassifierResult, PermissionClassifier, YoloClassifier};
pub use dangerous_patterns::{DangerLevel, DangerousPattern, detect_dangerous_patterns, has_dangerous_patterns, is_critically_dangerous};
pub use decision::{AllowDecision, AskDecision, DecisionReason, DenyDecision, PassthroughDecision, PermissionDecisionV2, PermissionUpdate, PermissionUpdateDestination};
pub use denial_tracking::{DenialTracker, SharedDenialTracker};
pub use explainer::explain_permission;
pub use filesystem::check_filesystem_permission;
pub use handler::{CoordinatorHandler, InteractiveHandler, PermissionCheckContext, PermissionHandler, SwarmWorkerHandler};
pub use loader::{load_rules_from_file, merge_rules, parse_rule_string};
pub use mode::{ExtendedPermissionMode, ModeColorKey, PermissionModeConfig};
pub use path_validation::validate_path;
pub use rule::{PermissionRuleV2, PermissionRuleValue};
pub use setup::{PermissionSetup, PermissionSetupConfig, get_next_permission_mode};
pub use shadowed_detection::{ShadowReason, ShadowedRule, detect_shadowed_rules};
pub use shell_matching::shell_command_matches_pattern;
