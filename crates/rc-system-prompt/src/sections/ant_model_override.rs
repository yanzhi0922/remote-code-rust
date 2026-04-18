//! Ant-only model override suffix section.
//!
//! External builds normally omit this section. It remains a first-class
//! section so the default section order matches Claude Code's prompt registry.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::SystemPromptSection;

pub struct AntModelOverrideSection;

impl SystemPromptSection for AntModelOverrideSection {
    fn name(&self) -> &str {
        "ant_model_override"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        Ok(None)
    }
}
