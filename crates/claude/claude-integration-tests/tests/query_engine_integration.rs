//! Query engine integration tests.
//!
//! Tests state machine transitions, budget tracker, failure tracker,
//! and observer events.

use std::time::Duration;

// ─── State machine transitions ──────────────────────────────────────────────

#[test]
fn state_machine_starts_at_idle() {
    let sm = claude_query_engine::state_machine::StateMachine::new();
    assert_eq!(
        sm.phase(),
        claude_query_engine::state_machine::EnginePhase::Idle
    );
}

#[test]
fn state_machine_idle_to_initializing() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();
    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("transition should succeed");
    assert_eq!(
        sm.phase(),
        claude_query_engine::state_machine::EnginePhase::Initializing
    );
}

#[test]
fn state_machine_full_lifecycle() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    // Idle → Initializing
    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("idle → initializing");
    assert_eq!(
        sm.phase(),
        claude_query_engine::state_machine::EnginePhase::Initializing
    );

    // Initializing → BuildingPrompt
    sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
        .expect("initializing → building_prompt");

    // BuildingPrompt → CallingProvider
    sm.transition(claude_query_engine::state_machine::EnginePhase::CallingProvider)
        .expect("building_prompt → calling_provider");

    // CallingProvider → ProcessingResponse
    sm.transition(claude_query_engine::state_machine::EnginePhase::ProcessingResponse)
        .expect("calling_provider → processing_response");

    // ProcessingResponse → ExecutingTools
    sm.transition(claude_query_engine::state_machine::EnginePhase::ExecutingTools)
        .expect("processing_response → executing_tools");

    // ExecutingTools → Finalizing
    sm.transition(claude_query_engine::state_machine::EnginePhase::Finalizing)
        .expect("executing_tools → finalizing");

    // Finalizing → Idle
    sm.transition(claude_query_engine::state_machine::EnginePhase::Idle)
        .expect("finalizing → idle");
}

#[test]
fn state_machine_tool_call_lifecycle() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::CallingProvider)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::ProcessingResponse)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::ExecutingTools)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Finalizing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Idle)
        .expect("ok");
}

#[test]
fn state_machine_invalid_transition() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();
    // Cannot go from Idle directly to CallingProvider
    let result = sm.transition(claude_query_engine::state_machine::EnginePhase::CallingProvider);
    assert!(result.is_err());
}

#[test]
fn state_machine_failure_path() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Failed)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Idle)
        .expect("ok");
}

#[test]
fn state_machine_cancel_path() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Cancelled)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Idle)
        .expect("ok");
}

#[test]
fn state_machine_compacting_path() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::Compacting)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
        .expect("ok");
}

#[test]
fn state_machine_records_transitions() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
        .expect("ok");
    sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
        .expect("ok");

    let transitions = sm.transitions();
    assert_eq!(transitions.len(), 2);
    assert_eq!(
        transitions[0].from,
        claude_query_engine::state_machine::EnginePhase::Idle
    );
    assert_eq!(
        transitions[0].to,
        claude_query_engine::state_machine::EnginePhase::Initializing
    );
    assert_eq!(
        transitions[1].from,
        claude_query_engine::state_machine::EnginePhase::Initializing
    );
    assert_eq!(
        transitions[1].to,
        claude_query_engine::state_machine::EnginePhase::BuildingPrompt
    );
}

#[test]
fn engine_phase_terminal_states() {
    assert!(claude_query_engine::state_machine::EnginePhase::Idle.is_terminal());
    assert!(claude_query_engine::state_machine::EnginePhase::Failed.is_terminal());
    assert!(claude_query_engine::state_machine::EnginePhase::Cancelled.is_terminal());
    assert!(!claude_query_engine::state_machine::EnginePhase::Initializing.is_terminal());
    assert!(!claude_query_engine::state_machine::EnginePhase::Compacting.is_terminal());
}

#[test]
fn engine_phase_active_states() {
    assert!(!claude_query_engine::state_machine::EnginePhase::Idle.is_active());
    assert!(claude_query_engine::state_machine::EnginePhase::Initializing.is_active());
    assert!(claude_query_engine::state_machine::EnginePhase::BuildingPrompt.is_active());
}

// ─── Budget tracker ─────────────────────────────────────────────────────────

#[test]
fn budget_tracker_allows_within_limits() {
    let tracker = claude_query_engine::BudgetTracker::new(10, Some(100000));
    assert_eq!(
        tracker.evaluate(0, 0),
        claude_query_engine::TokenBudgetDecision::Continue
    );
    assert_eq!(
        tracker.evaluate(5, 50000),
        claude_query_engine::TokenBudgetDecision::Continue
    );
}

#[test]
fn budget_tracker_stops_on_turn_limit() {
    let tracker = claude_query_engine::BudgetTracker::new(3, None);
    assert_eq!(
        tracker.evaluate(0, 0),
        claude_query_engine::TokenBudgetDecision::Continue
    );
    assert_eq!(
        tracker.evaluate(3, 0),
        claude_query_engine::TokenBudgetDecision::Stop {
            reason: "turn budget exceeded (3)".to_owned()
        }
    );
}

#[test]
fn budget_tracker_stops_on_token_limit() {
    let tracker = claude_query_engine::BudgetTracker::new(100, Some(1000));
    assert_eq!(
        tracker.evaluate(1, 1000),
        claude_query_engine::TokenBudgetDecision::Stop {
            reason: "token budget exceeded (1000)".to_owned()
        }
    );
}

#[test]
fn budget_tracker_no_token_limit() {
    let tracker = claude_query_engine::BudgetTracker::new(10, None);
    assert_eq!(
        tracker.evaluate(5, 999999999),
        claude_query_engine::TokenBudgetDecision::Continue
    );
}

#[test]
fn budget_tracker_serialization() {
    let tracker = claude_query_engine::BudgetTracker::new(5, Some(10000));
    let json = serde_json::to_string(&tracker).expect("serialize budget tracker");
    let decoded: claude_query_engine::BudgetTracker =
        serde_json::from_str(&json).expect("deserialize budget tracker");
    assert_eq!(decoded, tracker);
}

// ─── Failure tracker ────────────────────────────────────────────────────────

#[test]
fn failure_tracker_starts_closed() {
    let tracker = claude_query_engine::failure_tracker::FailureTracker::new(5, Duration::from_secs(30));
    assert_eq!(
        tracker.state(),
        claude_query_engine::failure_tracker::CircuitState::Closed
    );
    assert!(tracker.is_available());
}

#[test]
fn failure_tracker_accumulates_failures() {
    let mut tracker =
        claude_query_engine::failure_tracker::FailureTracker::new(3, Duration::from_secs(30));

    tracker.record_failure();
    assert_eq!(tracker.consecutive_failures(), 1);
    assert_eq!(tracker.total_failures(), 1);

    tracker.record_failure();
    tracker.record_failure();
    assert_eq!(tracker.consecutive_failures(), 3);
    assert_eq!(tracker.total_failures(), 3);
}

#[test]
fn failure_tracker_success_resets_consecutive() {
    let mut tracker =
        claude_query_engine::failure_tracker::FailureTracker::new(5, Duration::from_secs(30));

    tracker.record_failure();
    tracker.record_failure();
    assert_eq!(tracker.consecutive_failures(), 2);

    tracker.record_success();
    assert_eq!(tracker.consecutive_failures(), 0);
    assert_eq!(tracker.total_successes(), 1);
}

#[test]
fn failure_tracker_opens_on_threshold() {
    let mut tracker =
        claude_query_engine::failure_tracker::FailureTracker::new(2, Duration::from_secs(60));

    tracker.record_failure();
    assert_eq!(
        tracker.state(),
        claude_query_engine::failure_tracker::CircuitState::Closed
    );

    tracker.record_failure();
    // After reaching threshold, state should transition
    assert_eq!(
        tracker.state(),
        claude_query_engine::failure_tracker::CircuitState::Open
    );
}

#[test]
fn failure_tracker_max_failures_config() {
    let tracker =
        claude_query_engine::failure_tracker::FailureTracker::new(10, Duration::from_secs(30));
    assert_eq!(tracker.max_failures(), 10);
    assert_eq!(tracker.cooldown_duration(), Duration::from_secs(30));
}

// ─── Observer events ────────────────────────────────────────────────────────

#[test]
fn noop_observer_default() {
    let _observer = claude_query_engine::NoopQueryObserver;
}

#[test]
fn query_checkpoint_serialization() {
    let checkpoint = claude_query_engine::QueryCheckpoint::new(
        claude_query_engine::QueryCheckpointKind::ResumeBoundary,
        uuid::Uuid::new_v4().into(),
        0,
        None,
        vec![],
        0,
    );
    let json = serde_json::to_string(&checkpoint).expect("serialize checkpoint");
    assert!(!json.is_empty());
    let decoded: claude_query_engine::QueryCheckpoint =
        serde_json::from_str(&json).expect("deserialize checkpoint");
    assert_eq!(decoded, checkpoint);
}

// ─── Engine event round-trip via rc-engine-events ───────────────────────────

#[test]
fn engine_event_stream_started() {
    let event = rc_engine_events::EngineEvent::StreamStarted {
        request_id: "req-001".to_owned(),
    };
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("stream_started"));
}

#[test]
fn engine_event_tool_use_lifecycle() {
    // Tool use started
    let started = rc_engine_events::EngineEvent::ToolUseStarted {
        tool_use_id: "tu-1".to_owned(),
        tool_name: "read_file".to_owned(),
        input: serde_json::json!({"path": "/tmp/test.rs"}),
    };
    let json = serde_json::to_string(&started).expect("serialize");
    assert!(json.contains("tool_use_started"));

    // Tool use completed
    let completed = rc_engine_events::EngineEvent::ToolUseCompleted {
        tool_use_id: "tu-1".to_owned(),
        result: rc_engine_events::types::ToolResult {
            content: "file contents".to_owned(),
            is_error: false,
            mime_type: None,
        },
    };
    let json = serde_json::to_string(&completed).expect("serialize");
    assert!(json.contains("tool_use_completed"));
}

#[test]
fn engine_event_compact_lifecycle() {
    let started = rc_engine_events::EngineEvent::CompactStarted {
        strategy: "full".to_owned(),
    };
    let json = serde_json::to_string(&started).expect("serialize");
    assert!(json.contains("compact_started"));

    let completed = rc_engine_events::EngineEvent::CompactCompleted {
        result: rc_engine_events::CompactionResult {
            strategy: "full".to_owned(),
            before_messages: 50,
            after_messages: 25,
            summary: Some("compacted".to_owned()),
        },
    };
    let json = serde_json::to_string(&completed).expect("serialize");
    assert!(json.contains("compact_completed"));
}