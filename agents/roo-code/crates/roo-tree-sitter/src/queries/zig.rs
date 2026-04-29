/// Tree-sitter query for Zig language constructs.
///
/// Captures: functions, structs, enums, unions, constants, tests.
pub const QUERY: &str = r#"
; Function declarations
(function_declaration
  name: (identifier) @name.definition.function) @definition.function

; Test declarations
(test_declaration
  (identifier) @name.definition.test) @definition.test

; Top-level constant declarations
(top_level_declaration
  (identifier) @name.definition.constant) @definition.constant

; Container declarations (struct, enum, union, opaque)
(container_declaration
  (container_field) @name.definition.field) @definition.container

; Struct declarations
(struct_declaration
  name: (identifier) @name.definition.struct) @definition.struct

; Enum declarations
(enum_declaration
  name: (identifier) @name.definition.enum) @definition.enum

; Union declarations
(union_declaration
  name: (identifier) @name.definition.union) @definition.union

; Error set declarations
(error_set_declaration
  name: (identifier) @name.definition.error_set) @definition.error_set

; Variable declarations
(variable_declaration
  name: (identifier) @name.definition.variable) @definition.variable

; Using namespace declarations
(using_namespace_declaration) @definition.using_namespace
"#;
