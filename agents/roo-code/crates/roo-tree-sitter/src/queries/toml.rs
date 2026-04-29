/// Tree-sitter query for TOML language constructs.
///
/// Captures: tables, key-value pairs, arrays.
pub const QUERY: &str = r#"
; Standard tables
(table
  (dotted_key)? @name.definition.table
  (bare_key) @name.definition.table) @definition.table

; Array tables
(table_array
  (dotted_key)? @name.definition.array_table
  (bare_key) @name.definition.array_table) @definition.array_table

; Key-value pairs
(pair
  key: (bare_key) @name.definition.key) @definition.pair

; Dotted key pairs
(pair
  key: (dotted_key
    (bare_key) @name.definition.dotted_key)) @definition.dotted_pair
"#;
