/// Tree-sitter query for OCaml language constructs.
///
/// Captures: modules, types, functions, classes.
pub const QUERY: &str = r#"
; Module definitions
(module_definition
  (module_name) @name.definition.module) @definition.module

; Module type definitions
(module_type_definition
  (module_name) @name.definition.module_type) @definition.module_type

; Type definitions
(type_definition
  (type_binding
    (type_constructor) @name.definition.type)) @definition.type

; Value definitions
(value_definition
  (let_binding
    pattern: (value_name) @name.definition.function)) @definition.function

; Function definitions with parameters
(value_definition
  (let_binding
    pattern: (value_name) @name.definition.function
    (parameter)+)) @definition.function

; Exception definitions
(exception_definition
  (exception
    (type_constructor) @name.definition.exception)) @definition.exception

; Class definitions
(class_definition
  (class_binding
    (class_name) @name.definition.class)) @definition.class

; Class type definitions
(class_type_definition
  (class_type_binding
    (class_name) @name.definition.class_type)) @definition.class_type

; External declarations
(external_declaration
  (value_name) @name.definition.external) @definition.external

; Open declarations
(open_module) @definition.open

; Include declarations
(include_module) @definition.include
"#;
