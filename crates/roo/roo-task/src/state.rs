//! Task state machine.
//!
//! Provides [`StateMachine`] that manages task state transitions with
//! validation and event emission.

use crate::events::TaskEventEmitter;
use crate::types::{TaskError, TaskState};

// ---------------------------------------------------------------------------
// StateMachine
// ---------------------------------------------------------------------------

/// A state machine for task lifecycle management.
///
/// Enforces valid state transitions and emits events when state changes occur.
pub struct StateMachine {
    state: TaskState,
    emitter: TaskEventEmitter,
}

impl StateMachine {
    /// Create a new state machine starting in the `Idle` state.
    pub fn new() -> Self {
        Self {
            state: TaskState::Idle,
            emitter: TaskEventEmitter::new(),
        }
    }

    /// Create a state machine starting in the given state.
    pub fn with_initial_state(state: TaskState) -> Self {
        Self {
            state,
            emitter: TaskEventEmitter::new(),
        }
    }

    /// Get the current state.
    pub fn current(&self) -> TaskState {
        self.state
    }

    /// Attempt to transition to a new state.
    ///
    /// Returns `Ok(())` if the transition is valid, `Err` otherwise.
    /// Emits a `StateChanged` event on successful transition.
    pub fn transition_to(&mut self, target: TaskState) -> Result<TaskState, TaskError> {
        if self.state == target {
            return Ok(self.state);
        }

        if !self.state.can_transition_to(&target) {
            return Err(TaskError::InvalidTransition {
                from: self.state,
                to: target,
            });
        }

        let from = self.state;
        self.state = target;
        self.emitter.emit_state_changed(from, self.state);

        Ok(self.state)
    }

    /// Start the task (transition from Idle → Running).
    pub fn start(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Running)
    }

    /// Pause the task (transition from Running → Paused).
    pub fn pause(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Paused)
    }

    /// Resume the task (transition from Paused → Running).
    pub fn resume(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Running)
    }

    /// Complete the task (transition from Running → Completed).
    pub fn complete(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Completed)
    }

    /// Abort the task (transition from Running/Paused → Aborted).
    pub fn abort(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Aborted)
    }

    /// Delegate the task (transition from Running → Delegated).
    pub fn delegate(&mut self) -> Result<TaskState, TaskError> {
        self.transition_to(TaskState::Delegated)
    }

    /// Get a reference to the event emitter for registering listeners.
    pub fn emitter(&self) -> &TaskEventEmitter {
        &self.emitter
    }

    /// Get a mutable reference to the event emitter.
    pub fn emitter_mut(&mut self) -> &mut TaskEventEmitter {
        &mut self.emitter
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_starts_idle() {
        let sm = StateMachine::new();
        assert_eq!(sm.current(), TaskState::Idle);
    }

    #[test]
    fn test_state_machine_custom_initial_state() {
        let sm = StateMachine::with_initial_state(TaskState::Running);
        assert_eq!(sm.current(), TaskState::Running);
    }

    #[test]
    fn test_state_machine_start() {
        let mut sm = StateMachine::new();
        let result = sm.start().unwrap();
        assert_eq!(result, TaskState::Running);
        assert_eq!(sm.current(), TaskState::Running);
    }

    #[test]
    fn test_state_machine_full_lifecycle() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.complete().unwrap();
        assert_eq!(sm.current(), TaskState::Completed);
    }

    #[test]
    fn test_state_machine_pause_resume() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.pause().unwrap();
        assert_eq!(sm.current(), TaskState::Paused);
        sm.resume().unwrap();
        assert_eq!(sm.current(), TaskState::Running);
    }

    #[test]
    fn test_state_machine_abort_from_running() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.abort().unwrap();
        assert_eq!(sm.current(), TaskState::Aborted);
    }

    #[test]
    fn test_state_machine_abort_from_paused() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.pause().unwrap();
        sm.abort().unwrap();
        assert_eq!(sm.current(), TaskState::Aborted);
    }

    #[test]
    fn test_state_machine_delegate() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.delegate().unwrap();
        assert_eq!(sm.current(), TaskState::Delegated);
    }

    #[test]
    fn test_state_machine_invalid_transition_idle_to_completed() {
        let mut sm = StateMachine::new();
        let result = sm.complete();
        assert!(result.is_err());
        assert_eq!(sm.current(), TaskState::Idle);
    }

    #[test]
    fn test_state_machine_invalid_transition_completed_to_running() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        sm.complete().unwrap();
        let result = sm.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_state_machine_same_state_is_noop() {
        let mut sm = StateMachine::new();
        let result = sm.transition_to(TaskState::Idle);
        assert_eq!(result.unwrap(), TaskState::Idle);
    }

    #[test]
    fn test_state_machine_double_start_is_noop() {
        let mut sm = StateMachine::new();
        sm.start().unwrap();
        // Running → Running is a same-state no-op, not an error
        let result = sm.start();
        assert!(result.is_ok());
        assert_eq!(sm.current(), TaskState::Running);
    }
}
