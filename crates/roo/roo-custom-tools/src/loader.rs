use std::path::Path;

use crate::types::{CustomToolDefinition, CustomToolError, LoadResult};

/// Validate a custom tool definition.
///
/// Checks that:
/// - Name is non-empty
/// - Description is non-empty
/// - Parameters schema is a valid JSON object (or null)
pub fn validate_definition(definition: &CustomToolDefinition) -> Result<(), CustomToolError> {
    if definition.name.is_empty() {
        return Err(CustomToolError::InvalidDefinition(
            "Tool must have a non-empty name".to_string(),
        ));
    }

    if definition.description.is_empty() {
        return Err(CustomToolError::InvalidDefinition(
            "Tool must have a non-empty description".to_string(),
        ));
    }

    // Validate parameters_schema is an object or null
    match &definition.parameters_schema {
        serde_json::Value::Null => {}
        serde_json::Value::Object(_) => {}
        other => {
            return Err(CustomToolError::InvalidDefinition(format!(
                "parameters_schema must be an object or null, got: {}",
                other
            )));
        }
    }

    Ok(())
}

/// Load tool definitions from a directory.
///
/// Reads JSON and YAML files from the given directory, parses them
/// as `CustomToolDefinition`s, validates them, and returns a `LoadResult`.
///
/// Supported file extensions: `.json`, `.yaml`, `.yml`
pub fn load_from_directory(dir: &Path) -> Result<LoadResult, CustomToolError> {
    let mut result = LoadResult::default();

    if !dir.exists() {
        return Ok(result);
    }

    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if extension != "json" && extension != "yaml" && extension != "yml" {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

        match load_single_file(&path) {
            Ok(definition) => match validate_definition(&definition) {
                Ok(()) => {
                    result.loaded.push(definition.name.clone());
                }
                Err(e) => {
                    result.errors.push(format!("{file_name}: {e}"));
                }
            },
            Err(e) => {
                result.errors.push(format!("{file_name}: {e}"));
            }
        }
    }

    Ok(result)
}

/// Load a single tool definition from a file.
fn load_single_file(path: &Path) -> Result<CustomToolDefinition, CustomToolError> {
    let content = std::fs::read_to_string(path)?;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let definition: CustomToolDefinition = match extension {
        "json" => serde_json::from_str(&content)?,
        "yaml" | "yml" => serde_yaml::from_str(&content)?,
        _ => {
            return Err(CustomToolError::InvalidDefinition(format!(
                "Unsupported file extension: {extension}"
            )));
        }
    };

    Ok(definition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HandlerType;
    use serde_json::json;
    use std::io::Write;

    fn make_valid_definition() -> CustomToolDefinition {
        CustomToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters_schema: json!({"type": "object"}),
            handler_type: HandlerType::Builtin,
        }
    }

    #[test]
    fn test_validate_valid_definition() {
        let def = make_valid_definition();
        assert!(validate_definition(&def).is_ok());
    }

    #[test]
    fn test_validate_empty_name() {
        let mut def = make_valid_definition();
        def.name = "".to_string();
        let result = validate_definition(&def);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-empty name"));
    }

    #[test]
    fn test_validate_empty_description() {
        let mut def = make_valid_definition();
        def.description = "".to_string();
        let result = validate_definition(&def);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-empty description"));
    }

    #[test]
    fn test_validate_null_parameters_schema() {
        let mut def = make_valid_definition();
        def.parameters_schema = json!(null);
        assert!(validate_definition(&def).is_ok());
    }

    #[test]
    fn test_validate_invalid_parameters_schema() {
        let mut def = make_valid_definition();
        def.parameters_schema = json!("not an object");
        let result = validate_definition(&def);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_array_parameters_schema() {
        let mut def = make_valid_definition();
        def.parameters_schema = json!([1, 2, 3]);
        let result = validate_definition(&def);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_nonexistent_directory() {
        let result = load_from_directory(Path::new("/nonexistent/path")).unwrap();
        assert!(result.loaded.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_load_from_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_from_directory(dir.path()).unwrap();
        assert!(result.loaded.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_load_from_directory_with_json_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("tool.json");
        let mut file = std::fs::File::create(&tool_path).unwrap();
        let def = make_valid_definition();
        write!(file, "{}", serde_json::to_string(&def).unwrap()).unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert_eq!(result.loaded.len(), 1);
        assert!(result.loaded.contains(&"test_tool".to_string()));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_load_from_directory_with_yaml_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("tool.yaml");
        let mut file = std::fs::File::create(&tool_path).unwrap();
        let def = make_valid_definition();
        write!(file, "{}", serde_yaml::to_string(&def).unwrap()).unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert_eq!(result.loaded.len(), 1);
        assert!(result.loaded.contains(&"test_tool".to_string()));
    }

    #[test]
    fn test_load_from_directory_with_yml_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("tool.yml");
        let mut file = std::fs::File::create(&tool_path).unwrap();
        let def = make_valid_definition();
        write!(file, "{}", serde_yaml::to_string(&def).unwrap()).unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert_eq!(result.loaded.len(), 1);
    }

    #[test]
    fn test_load_from_directory_skips_non_tool_files() {
        let dir = tempfile::tempdir().unwrap();
        let txt_path = dir.path().join("readme.txt");
        std::fs::write(&txt_path, "not a tool").unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert!(result.loaded.is_empty());
    }

    #[test]
    fn test_load_from_directory_with_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("bad.json");
        std::fs::write(&tool_path, "not valid json").unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert!(result.loaded.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_load_from_directory_with_invalid_tool() {
        let dir = tempfile::tempdir().unwrap();
        let tool_path = dir.path().join("invalid.json");
        let mut def = make_valid_definition();
        def.name = "".to_string(); // Invalid: empty name
        std::fs::write(&tool_path, serde_json::to_string(&def).unwrap()).unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert!(result.loaded.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_load_from_directory_mixed_files() {
        let dir = tempfile::tempdir().unwrap();

        // Valid tool
        let valid_path = dir.path().join("valid.json");
        let valid_def = make_valid_definition();
        std::fs::write(&valid_path, serde_json::to_string(&valid_def).unwrap()).unwrap();

        // Invalid tool
        let invalid_path = dir.path().join("invalid.json");
        std::fs::write(&invalid_path, "bad json").unwrap();

        // Non-tool file
        let txt_path = dir.path().join("readme.txt");
        std::fs::write(&txt_path, "text").unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert_eq!(result.loaded.len(), 1);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_load_from_directory_skips_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        let result = load_from_directory(dir.path()).unwrap();
        assert!(result.loaded.is_empty());
        assert!(result.errors.is_empty());
    }
}
