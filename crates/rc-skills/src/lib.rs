use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDescriptor {
    pub name: String,
    pub path: PathBuf,
}

pub fn discover_skills(root: &Path) -> Result<Vec<SkillDescriptor>> {
    let mut skills = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || entry.file_name() != "SKILL.md" {
            continue;
        }
        let name = entry
            .path()
            .parent()
            .and_then(Path::file_name)
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown-skill".to_owned());
        skills.push(SkillDescriptor {
            name,
            path: entry.path().to_path_buf(),
        });
    }
    Ok(skills)
}
