//! Environment types and listing.
//!
//! Defines the `Environment` struct, status enum, and `EnvironmentList`
//! with filtering and sorting capabilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EnvironmentStatus
// ---------------------------------------------------------------------------

/// Status of a teleport target environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentStatus {
    /// Environment is online and accepting sessions.
    Available,
    /// Environment is currently processing another session.
    Busy,
    /// Environment is not reachable.
    Offline,
}

impl EnvironmentStatus {
    /// Returns `true` if the environment can accept new sessions.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns a string label for display purposes.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Busy => "Busy",
            Self::Offline => "Offline",
        }
    }
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// A teleport target environment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Environment {
    /// Unique environment identifier.
    pub id: String,
    /// Human-readable environment name.
    pub name: String,
    /// Hostname or address of the environment.
    pub host: String,
    /// Current status of the environment.
    pub status: EnvironmentStatus,
    /// When the environment was last active.
    pub last_active: DateTime<Utc>,
}

impl Environment {
    /// Create a new environment with the given ID and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            host: String::new(),
            status: EnvironmentStatus::Available,
            last_active: Utc::now(),
        }
    }

    /// Set the host of this environment.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the status of this environment.
    pub fn with_status(mut self, status: EnvironmentStatus) -> Self {
        self.status = status;
        self
    }

    /// Check if this environment is currently available for teleportation.
    pub fn is_available(&self) -> bool {
        self.status.is_available()
    }
}

// ---------------------------------------------------------------------------
// EnvironmentList
// ---------------------------------------------------------------------------

/// A collection of environments with filtering and sorting support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EnvironmentList {
    /// The environments in this list.
    pub environments: Vec<Environment>,
}

impl EnvironmentList {
    /// Create a new empty environment list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an environment list from a vector of environments.
    pub fn from_vec(environments: Vec<Environment>) -> Self {
        Self { environments }
    }

    /// Add an environment to the list.
    pub fn push(&mut self, env: Environment) {
        self.environments.push(env);
    }

    /// Filter environments by status.
    pub fn filter_by_status(&self, status: EnvironmentStatus) -> EnvironmentList {
        let filtered = self
            .environments
            .iter()
            .filter(|e| e.status == status)
            .cloned()
            .collect();
        Self::from_vec(filtered)
    }

    /// Filter environments by name substring (case-insensitive).
    pub fn filter_by_name(&self, name_filter: &str) -> EnvironmentList {
        let filter_lower = name_filter.to_lowercase();
        let filtered = self
            .environments
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&filter_lower))
            .cloned()
            .collect();
        Self::from_vec(filtered)
    }

    /// Sort environments by last active time (most recent first).
    pub fn sort_by_last_active(&mut self) {
        self.environments
            .sort_by(|a, b| b.last_active.cmp(&a.last_active));
    }

    /// Sort environments by name (alphabetical).
    pub fn sort_by_name(&mut self) {
        self.environments.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Get only available environments.
    pub fn available(&self) -> EnvironmentList {
        self.filter_by_status(EnvironmentStatus::Available)
    }

    /// Number of environments in the list.
    pub fn len(&self) -> usize {
        self.environments.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.environments.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_env(id: &str, name: &str, status: EnvironmentStatus) -> Environment {
        Environment {
            id: id.to_string(),
            name: name.to_string(),
            host: format!("{id}.example.com"),
            status,
            last_active: Utc::now(),
        }
    }

    #[test]
    fn environment_status_checks() {
        assert!(EnvironmentStatus::Available.is_available());
        assert!(!EnvironmentStatus::Busy.is_available());
        assert!(!EnvironmentStatus::Offline.is_available());
    }

    #[test]
    fn environment_status_labels() {
        assert_eq!(EnvironmentStatus::Available.label(), "Available");
        assert_eq!(EnvironmentStatus::Busy.label(), "Busy");
        assert_eq!(EnvironmentStatus::Offline.label(), "Offline");
    }

    #[test]
    fn environment_builder_pattern() {
        let env = Environment::new("env-1", "Test Env")
            .with_host("host.example.com")
            .with_status(EnvironmentStatus::Busy);
        assert_eq!(env.id, "env-1");
        assert_eq!(env.name, "Test Env");
        assert_eq!(env.host, "host.example.com");
        assert_eq!(env.status, EnvironmentStatus::Busy);
        assert!(!env.is_available());
    }

    #[test]
    fn environment_list_filter_by_status() {
        let list = EnvironmentList::from_vec(vec![
            test_env("1", "Available Env", EnvironmentStatus::Available),
            test_env("2", "Busy Env", EnvironmentStatus::Busy),
            test_env("3", "Offline Env", EnvironmentStatus::Offline),
            test_env("4", "Another Available", EnvironmentStatus::Available),
        ]);
        let available = list.filter_by_status(EnvironmentStatus::Available);
        assert_eq!(available.len(), 2);
        assert!(available.environments.iter().all(|e| e.is_available()));
    }

    #[test]
    fn environment_list_filter_by_name() {
        let list = EnvironmentList::from_vec(vec![
            test_env("1", "Production Server", EnvironmentStatus::Available),
            test_env("2", "Staging Server", EnvironmentStatus::Available),
            test_env("3", "Dev Machine", EnvironmentStatus::Available),
        ]);
        let filtered = list.filter_by_name("server");
        assert_eq!(filtered.len(), 2);
        let filtered_case = list.filter_by_name("SERVER");
        assert_eq!(filtered_case.len(), 2);
    }

    #[test]
    fn environment_list_sort_by_name() {
        let mut list = EnvironmentList::from_vec(vec![
            test_env("1", "Charlie", EnvironmentStatus::Available),
            test_env("2", "Alice", EnvironmentStatus::Available),
            test_env("3", "Bob", EnvironmentStatus::Available),
        ]);
        list.sort_by_name();
        let names: Vec<&str> = list.environments.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn environment_list_available_shortcut() {
        let list = EnvironmentList::from_vec(vec![
            test_env("1", "Env 1", EnvironmentStatus::Available),
            test_env("2", "Env 2", EnvironmentStatus::Offline),
        ]);
        assert_eq!(list.available().len(), 1);
    }

    #[test]
    fn environment_serialization_roundtrip() {
        let env = test_env("env-42", "Test", EnvironmentStatus::Busy);
        let json = serde_json::to_string(&env).expect("serialize");
        let parsed: Environment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(env.id, parsed.id);
        assert_eq!(env.status, parsed.status);
    }
}
