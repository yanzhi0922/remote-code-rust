use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub manifest_path: PathBuf,
}

pub fn discover_plugins(root: &Path) -> Result<Vec<PluginManifest>> {
    let mut plugins = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || entry.file_name() != "plugin.json" {
            continue;
        }
        plugins.push(PluginManifest {
            name: entry
                .path()
                .parent()
                .and_then(Path::file_name)
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown-plugin".to_owned()),
            manifest_path: entry.path().to_path_buf(),
        });
    }
    Ok(plugins)
}
