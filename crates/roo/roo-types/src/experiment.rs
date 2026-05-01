//! Experiment type definitions.
//!
//! Derived from `packages/types/src/experiment.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ExperimentId
// ---------------------------------------------------------------------------

/// All experiment IDs.
///
/// Source: `packages/types/src/experiment.ts` — `experimentIds`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExperimentId {
    PreventFocusDisruption,
    ImageGeneration,
    RunSlashCommand,
    CustomTools,
}

// ---------------------------------------------------------------------------
// Experiments
// ---------------------------------------------------------------------------

/// Experiment flags.
///
/// Source: `packages/types/src/experiment.ts` — `experimentsSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Experiments {
    pub prevent_focus_disruption: Option<bool>,
    pub image_generation: Option<bool>,
    pub run_slash_command: Option<bool>,
    pub custom_tools: Option<bool>,
}
