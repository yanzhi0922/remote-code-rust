//! Language-specific tree-sitter query strings.
//!
//! Each module exports a `QUERY` constant containing the tree-sitter query
//! pattern for that language. These are direct copies from the TypeScript
//! source code's `.ts` query files.

pub mod c;
pub mod cpp;
pub mod csharp;
pub mod css;
pub mod elisp;
pub mod elixir;
pub mod embedded_template;
pub mod go;
pub mod html;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod lua;
pub mod ocaml;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod solidity;
pub mod swift;
pub mod systemrdl;
pub mod tlaplus;
pub mod toml;
pub mod tsx;
pub mod typescript;
pub mod vue;
pub mod zig;

/// Returns the query string for a given file extension (without the leading dot).
///
/// Maps extensions like "rs", "ts", "py" to their corresponding query strings.
/// Returns `None` for unsupported extensions.
pub fn query_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "js" | "jsx" | "json" => Some(javascript::QUERY),
        "ts" => Some(typescript::QUERY),
        "tsx" => Some(tsx::QUERY),
        "py" => Some(python::QUERY),
        "rs" => Some(rust::QUERY),
        "go" => Some(go::QUERY),
        "cpp" | "hpp" => Some(cpp::QUERY),
        "c" | "h" => Some(c::QUERY),
        "cs" => Some(csharp::QUERY),
        "rb" => Some(ruby::QUERY),
        "java" => Some(java::QUERY),
        "php" => Some(php::QUERY),
        "swift" => Some(swift::QUERY),
        "kt" | "kts" => Some(kotlin::QUERY),
        "css" => Some(css::QUERY),
        "html" | "htm" => Some(html::QUERY),
        "ml" | "mli" => Some(ocaml::QUERY),
        "scala" => Some(lua::QUERY), // Temporarily use Lua query (matches TS behavior)
        "sol" => Some(solidity::QUERY),
        "toml" => Some(toml::QUERY),
        "vue" => Some(vue::QUERY),
        "lua" => Some(lua::QUERY),
        "rdl" => Some(systemrdl::QUERY),
        "tla" => Some(tlaplus::QUERY),
        "zig" => Some(zig::QUERY),
        "ejs" | "erb" => Some(embedded_template::QUERY),
        "el" => Some(elisp::QUERY),
        "ex" | "exs" => Some(elixir::QUERY),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_for_extension_rust() {
        assert!(query_for_extension("rs").is_some());
        assert!(query_for_extension("rs").unwrap().contains("function_item"));
    }

    #[test]
    fn test_query_for_extension_typescript() {
        assert!(query_for_extension("ts").is_some());
        assert!(query_for_extension("tsx").is_some());
    }

    #[test]
    fn test_query_for_extension_unknown() {
        assert!(query_for_extension("xyz").is_none());
    }

    #[test]
    fn test_query_for_extension_javascript_aliases() {
        assert!(query_for_extension("js").is_some());
        assert!(query_for_extension("jsx").is_some());
        assert!(query_for_extension("json").is_some());
        // All should return the same javascript query
        assert_eq!(query_for_extension("js"), query_for_extension("jsx"));
    }

    #[test]
    fn test_query_for_extension_c_cpp() {
        assert!(query_for_extension("c").is_some());
        assert!(query_for_extension("h").is_some());
        assert!(query_for_extension("cpp").is_some());
        assert!(query_for_extension("hpp").is_some());
    }
}
