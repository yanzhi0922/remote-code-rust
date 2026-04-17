//! Settings merging across layers.
//!
//! Corresponds to the layered settings merge logic in the TS source.

use serde::{Deserialize, Serialize};

use crate::types::Settings;

/// The source of a settings layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsLayer {
    /// Global settings (~/.claude/settings.json).
    Global,
    /// Project settings (.claude/settings.json).
    Project,
    /// Local settings (.claude/settings.local.json).
    Local,
    /// Managed/policy settings (enterprise).
    Managed,
}

/// Merge multiple settings layers into a single effective settings.
///
/// Priority (highest to lowest): Managed > Local > Project > Global.
/// For most fields, the highest-priority non-None value wins.
/// For arrays (allow, deny, etc.), values are merged/concatenated.
pub fn merge_settings(layers: &[(SettingsLayer, Settings)]) -> Settings {
    let mut result = Settings::default();

    // Process in order: Global → Project → Local → Managed
    let order = [
        SettingsLayer::Global,
        SettingsLayer::Project,
        SettingsLayer::Local,
        SettingsLayer::Managed,
    ];

    for layer_type in &order {
        for (layer, settings) in layers {
            if layer == layer_type {
                merge_into(&mut result, settings);
            }
        }
    }

    result
}

/// Merge source settings into target, with source taking precedence.
fn merge_into(target: &mut Settings, source: &Settings) {
    // Simple fields: source overrides target if present
    if source.model.is_some() {
        target.model = source.model.clone();
    }
    if source.provider.is_some() {
        target.provider = source.provider.clone();
    }
    if source.api_key_helper.is_some() {
        target.api_key_helper = source.api_key_helper.clone();
    }
    if source.effort_level.is_some() {
        target.effort_level = source.effort_level.clone();
    }
    if source.fast_mode.is_some() {
        target.fast_mode = source.fast_mode;
    }
    if source.always_thinking_enabled.is_some() {
        target.always_thinking_enabled = source.always_thinking_enabled;
    }
    if source.default_shell.is_some() {
        target.default_shell = source.default_shell.clone();
    }
    if source.output_style.is_some() {
        target.output_style = source.output_style.clone();
    }
    if source.language.is_some() {
        target.language = source.language.clone();
    }
    if source.auto_updates_channel.is_some() {
        target.auto_updates_channel = source.auto_updates_channel.clone();
    }
    if source.cleanup_period_days.is_some() {
        target.cleanup_period_days = source.cleanup_period_days;
    }
    if source.file_checkpointing_enabled.is_some() {
        target.file_checkpointing_enabled = source.file_checkpointing_enabled;
    }
    if source.disable_all_hooks.is_some() {
        target.disable_all_hooks = source.disable_all_hooks;
    }
    if source.enable_all_project_mcp_servers.is_some() {
        target.enable_all_project_mcp_servers = source.enable_all_project_mcp_servers;
    }

    // Merge permissions (concatenate arrays)
    if let Some(source_perms) = &source.permissions {
        let target_perms = target.permissions.get_or_insert_with(Default::default);

        merge_string_vec(&mut target_perms.allow, &source_perms.allow);
        merge_string_vec(&mut target_perms.deny, &source_perms.deny);
        merge_string_vec(&mut target_perms.ask, &source_perms.ask);

        if source_perms.default_mode.is_some() {
            target_perms.default_mode = source_perms.default_mode.clone();
        }
    }

    // Merge env (source overrides target keys)
    if let Some(source_env) = &source.env {
        let target_env = target.env.get_or_insert_with(Default::default);
        for (k, v) in source_env {
            target_env.insert(k.clone(), v.clone());
        }
    }

    // Merge providers
    if let Some(source_providers) = &source.providers {
        let target_providers = target.providers.get_or_insert_with(Default::default);
        for (k, v) in source_providers {
            target_providers.insert(k.clone(), v.clone());
        }
    }

    // Merge arrays (concatenate)
    merge_string_vec(
        &mut target.enabled_mcpjson_servers,
        &source.enabled_mcpjson_servers,
    );
    merge_string_vec(
        &mut target.disabled_mcpjson_servers,
        &source.disabled_mcpjson_servers,
    );
    merge_string_vec(
        &mut target.allowed_http_hook_urls,
        &source.allowed_http_hook_urls,
    );
    merge_string_vec(
        &mut target.http_hook_allowed_env_vars,
        &source.http_hook_allowed_env_vars,
    );
    merge_string_vec(&mut target.available_models, &source.available_models);
    merge_string_vec(
        &mut target.company_announcements,
        &source.company_announcements,
    );
}

/// Merge two optional string vectors by concatenating.
fn merge_string_vec(target: &mut Option<Vec<String>>, source: &Option<Vec<String>>) {
    if let Some(src) = source {
        let tgt = target.get_or_insert_with(Vec::new);
        for item in src {
            if !tgt.contains(item) {
                tgt.push(item.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::PermissionSettings;

    #[test]
    fn merge_empty_layers() {
        let result = merge_settings(&[]);
        assert!(result.model.is_none());
    }

    #[test]
    fn merge_global_only() {
        let global = Settings {
            model: Some("claude-sonnet".to_string()),
            ..Default::default()
        };
        let result = merge_settings(&[(SettingsLayer::Global, global)]);
        assert_eq!(result.model.as_deref(), Some("claude-sonnet"));
    }

    #[test]
    fn merge_project_overrides_global() {
        let global = Settings {
            model: Some("global-model".to_string()),
            ..Default::default()
        };
        let project = Settings {
            model: Some("project-model".to_string()),
            ..Default::default()
        };
        let result = merge_settings(&[
            (SettingsLayer::Global, global),
            (SettingsLayer::Project, project),
        ]);
        assert_eq!(result.model.as_deref(), Some("project-model"));
    }

    #[test]
    fn merge_managed_overrides_all() {
        let global = Settings {
            model: Some("global".to_string()),
            ..Default::default()
        };
        let project = Settings {
            model: Some("project".to_string()),
            ..Default::default()
        };
        let managed = Settings {
            model: Some("managed".to_string()),
            ..Default::default()
        };
        let result = merge_settings(&[
            (SettingsLayer::Global, global),
            (SettingsLayer::Project, project),
            (SettingsLayer::Managed, managed),
        ]);
        assert_eq!(result.model.as_deref(), Some("managed"));
    }

    #[test]
    fn merge_permissions_concatenates() {
        let global = Settings {
            permissions: Some(PermissionSettings {
                allow: Some(vec!["Bash(*)".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let project = Settings {
            permissions: Some(PermissionSettings {
                allow: Some(vec!["Edit(*)".to_string()]),
                deny: Some(vec!["Bash(rm -rf /)".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = merge_settings(&[
            (SettingsLayer::Global, global),
            (SettingsLayer::Project, project),
        ]);
        let perms = result.permissions.unwrap();
        assert!(perms.is_allowed("Bash(*)"));
        assert!(perms.is_allowed("Edit(*)"));
        assert!(perms.is_denied("Bash(rm -rf /)"));
    }

    #[test]
    fn merge_env_overrides() {
        let global = Settings {
            env: Some(
                vec![
                    ("KEY1".to_string(), "global1".to_string()),
                    ("KEY2".to_string(), "global2".to_string()),
                ]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        };
        let project = Settings {
            env: Some(
                vec![("KEY2".to_string(), "project2".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };
        let result = merge_settings(&[
            (SettingsLayer::Global, global),
            (SettingsLayer::Project, project),
        ]);
        let env = result.env.unwrap();
        assert_eq!(env.get("KEY1").unwrap(), "global1");
        assert_eq!(env.get("KEY2").unwrap(), "project2");
    }

    #[test]
    fn merge_no_duplicate_arrays() {
        let global = Settings {
            enabled_mcpjson_servers: Some(vec!["server-a".to_string()]),
            ..Default::default()
        };
        let project = Settings {
            enabled_mcpjson_servers: Some(vec!["server-a".to_string(), "server-b".to_string()]),
            ..Default::default()
        };
        let result = merge_settings(&[
            (SettingsLayer::Global, global),
            (SettingsLayer::Project, project),
        ]);
        let servers = result.enabled_mcpjson_servers.unwrap();
        assert_eq!(servers.len(), 2); // No duplicate "server-a"
    }
}
