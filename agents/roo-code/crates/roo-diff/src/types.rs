/// Result of applying a diff operation.
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
    pub fail_parts: Vec<DiffResult>,
}

impl DiffResult {
    /// Create a successful diff result with the given content.
    pub fn ok(content: String, fail_parts: Vec<DiffResult>) -> Self {
        Self {
            success: true,
            content: Some(content),
            error: None,
            fail_parts,
        }
    }

    /// Create a failed diff result with an error message.
    pub fn fail(error: String) -> Self {
        Self {
            success: false,
            content: None,
            error: Some(error),
            fail_parts: Vec::new(),
        }
    }

    /// Create a failed diff result with fail parts but no single error.
    pub fn fail_with_parts(fail_parts: Vec<DiffResult>) -> Self {
        Self {
            success: false,
            content: None,
            error: None,
            fail_parts,
        }
    }
}

/// Represents a tool use with its parameters.
#[derive(Debug, Clone)]
pub struct ToolUse {
    pub params: ToolUseParams,
    pub partial: bool,
}

/// Parameters for a tool use, containing the diff content.
#[derive(Debug, Clone)]
pub struct ToolUseParams {
    pub diff: Option<String>,
}

/// Progress status for tool operations.
#[derive(Debug, Clone)]
pub struct ToolProgressStatus {
    pub icon: Option<String>,
    pub text: Option<String>,
}
