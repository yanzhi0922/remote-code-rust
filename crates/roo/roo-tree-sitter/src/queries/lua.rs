/// Tree-sitter query for Lua language constructs.
///
/// Captures: functions, tables, local assignments, modules.
pub const QUERY: &str = r#"
; Function declarations
(function_declaration
  name: [(identifier) @name.definition.function
    (dot_index_expression
      field: (identifier) @name.definition.method)
    (colon_index_expression
      field: (identifier) @name.definition.method)]) @definition.function

; Local function declarations
(function_declaration
  (local)
  name: (identifier) @name.definition.function) @definition.local_function

; Anonymous functions assigned to variables
(assignment_statement
  (variable_list
    name: (identifier) @name.definition.function)
  (expression_list
    value: (function_definition))) @definition.function

; Local variable assignments with function values
(local_assignment
  (variable_list
    name: (identifier) @name.definition.function)
  (expression_list
    value: (function_definition))) @definition.local_function

; Table definitions
(table_constructor) @definition.table

; Table fields
(field
  name: (identifier) @name.definition.field) @definition.field

; Method definitions
(function_declaration
  name: (colon_index_expression
    field: (identifier) @name.definition.method)) @definition.method

; Module requires
(function_call
  name: (identifier) @name.definition.require
  arguments: (arguments
    (string) @name.definition.module))
  (#eq? @name.definition.require "require")
"#;
