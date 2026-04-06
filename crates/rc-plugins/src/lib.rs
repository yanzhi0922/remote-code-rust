use std::{
    fs,
    path::{Path, PathBuf},
};

use rc_mcp::McpConfig;
use rc_skills::{SkillDocument, SkillError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use walkdir::WalkDir;

pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";
pub const PLUGIN_MANIFEST_DIR: &str = ".codex-plugin";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub skills: Option<String>,
    #[serde(default)]
    pub apps: Option<String>,
    #[serde(default)]
    pub mcp: Option<String>,
    #[serde(default)]
    pub interface: Option<PluginInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInterface {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "shortDescription")]
    pub short_description: String,
    #[serde(rename = "longDescription")]
    pub long_description: Option<String>,
    #[serde(rename = "developerName")]
    pub developer_name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(rename = "websiteURL")]
    pub website_url: Option<String>,
    #[serde(rename = "privacyPolicyURL")]
    pub privacy_policy_url: Option<String>,
    #[serde(rename = "termsOfServiceURL")]
    pub terms_of_service_url: Option<String>,
    #[serde(rename = "defaultPrompt", default)]
    pub default_prompt: Vec<String>,
    #[serde(rename = "composerIcon")]
    pub composer_icon: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(rename = "brandColor")]
    pub brand_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCapability {
    Read,
    Write,
    Interactive,
    Background,
    Network,
    Unknown(String),
}

impl Serialize for PluginCapability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Interactive => "Interactive",
            Self::Background => "Background",
            Self::Network => "Network",
            Self::Unknown(value) => value,
        })
    }
}

impl<'de> Deserialize<'de> for PluginCapability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "Read" => Self::Read,
            "Write" => Self::Write,
            "Interactive" => Self::Interactive,
            "Background" => Self::Background,
            "Network" => Self::Network,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginBundle {
    pub manifest: PluginManifest,
    pub manifest_path: PathBuf,
    pub root: PathBuf,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("failed to read plugin manifest `{path}`")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse plugin manifest `{path}`")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub fn discover_plugins(root: &Path) -> Result<Vec<PluginBundle>, PluginError> {
    let mut plugins = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == PLUGIN_MANIFEST_FILE)
        .map(|entry| load_plugin(entry.path()))
        .collect::<Result<Vec<_>, _>>()?;

    plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    Ok(plugins)
}

pub fn load_plugin(path: impl AsRef<Path>) -> Result<PluginBundle, PluginError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| PluginError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest = serde_json::from_str(&content).map_err(|source| PluginError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(PluginBundle {
        manifest,
        manifest_path: path.to_path_buf(),
        root: resolve_plugin_root(path),
    })
}

impl PluginBundle {
    #[must_use]
    pub fn skills_root(&self) -> Option<PathBuf> {
        self.manifest
            .skills
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    #[must_use]
    pub fn app_manifest_path(&self) -> Option<PathBuf> {
        self.manifest
            .apps
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    #[must_use]
    pub fn mcp_config_path(&self) -> Option<PathBuf> {
        self.manifest
            .mcp
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    pub fn discover_bundled_skills(&self) -> Result<Vec<SkillDocument>, SkillError> {
        match self.skills_root() {
            Some(root) => rc_skills::discover_skills(&root),
            None => Ok(Vec::new()),
        }
    }

    pub fn load_mcp_config(&self) -> Result<Option<McpConfig>, rc_mcp::McpConfigError> {
        match self.mcp_config_path() {
            Some(path) => McpConfig::load(path).map(Some),
            None => Ok(None),
        }
    }

    fn resolve_relative(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

fn resolve_plugin_root(manifest_path: &Path) -> PathBuf {
    let Some(parent) = manifest_path.parent() else {
        return PathBuf::from(".");
    };
    if parent
        .file_name()
        .is_some_and(|name| name == PLUGIN_MANIFEST_DIR)
    {
        return parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| parent.to_path_buf());
    }
    parent.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn loads_plugin_manifest_and_resolves_paths() {
        let temp = ok(tempdir());
        let root = temp.path().join("github-plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{
                "name": "github",
                "version": "0.1.0",
                "skills": "./skills",
                "apps": "./.app.json",
                "mcp": "./mcp.toml",
                "interface": {
                    "displayName": "GitHub",
                    "shortDescription": "Triage GitHub work",
                    "capabilities": ["Interactive", "Write", "ExperimentalCapability"],
                    "defaultPrompt": ["Help with GitHub"]
                }
            }"#,
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));

        assert_eq!(plugin.root, root);
        assert_eq!(plugin.manifest.name, "github");
        assert_eq!(plugin.skills_root(), Some(plugin.root.join("./skills")));
        assert_eq!(
            plugin.app_manifest_path(),
            Some(plugin.root.join("./.app.json"))
        );
        assert_eq!(
            plugin.mcp_config_path(),
            Some(plugin.root.join("./mcp.toml"))
        );
        let interface = match plugin.manifest.interface {
            Some(interface) => interface,
            None => panic!("missing interface"),
        };
        assert_eq!(
            interface.capabilities,
            vec![
                PluginCapability::Interactive,
                PluginCapability::Write,
                PluginCapability::Unknown("ExperimentalCapability".to_owned())
            ]
        );
    }

    #[test]
    fn discovers_plugins_sorted_by_name() {
        let temp = ok(tempdir());
        let alpha = temp.path().join("alpha");
        let zeta = temp.path().join("zeta");
        ok(fs::create_dir_all(alpha.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::create_dir_all(zeta.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            alpha.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"alpha","version":"0.1.0"}"#,
        ));
        ok(fs::write(
            zeta.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"zeta","version":"0.1.0"}"#,
        ));

        let plugins = ok(discover_plugins(temp.path()));

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].manifest.name, "alpha");
        assert_eq!(plugins[1].manifest.name, "zeta");
    }

    #[test]
    fn discovers_bundled_skills() {
        let temp = ok(tempdir());
        let root = temp.path().join("bundle");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::create_dir_all(root.join("skills").join("demo")));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"bundle","version":"0.1.0","skills":"./skills"}"#,
        ));
        ok(fs::write(
            root.join("skills").join("demo").join("SKILL.md"),
            "# Demo\n\nDemo summary.\n",
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let skills = ok(plugin.discover_bundled_skills());

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].metadata.slug, "demo");
    }

    #[test]
    fn loads_plugin_mcp_config() {
        let temp = ok(tempdir());
        let root = temp.path().join("mcp-plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"mcp-plugin","version":"0.1.0","mcp":"./mcp.toml"}"#,
        ));
        ok(fs::write(
            root.join("mcp.toml"),
            "[mcp_servers.demo]\ncommand = \"uvx\"\n",
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let config = ok(plugin.load_mcp_config());
        let config = match config {
            Some(config) => config,
            None => panic!("missing MCP config"),
        };

        assert!(config.servers.contains_key("demo"));
    }
}
