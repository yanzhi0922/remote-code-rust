/// Tree-sitter query for TLA+ language constructs.
///
/// Captures: modules, operators, variables, constants, assumptions.
pub const QUERY: &str = r#"
; Module definitions
(module
  header: (identifier) @name.definition.module) @definition.module

; Constant declarations
(constant_declaration
  (identifier) @name.definition.constant) @definition.constant

; Variable declarations
(variable_declaration
  (identifier) @name.definition.variable) @definition.variable

; Operator definitions
(operator_definition
  name: (identifier) @name.definition.operator) @definition.operator

; Function definitions
(function_definition
  name: (identifier) @name.definition.function) @definition.function

; Theorem definitions
(theorem
  name: (identifier) @name.definition.theorem) @definition.theorem

; Assumption definitions
(assumption
  name: (identifier) @name.definition.assumption) @definition.assumption

; Instance definitions
(instance
  name: (identifier) @name.definition.instance) @definition.instance

; Module definitions (recursive)
(module_definition
  name: (identifier) @name.definition.module_def) @definition.module_def

; Type definitions
(type_definition
  name: (identifier) @name.definition.type) @definition.type
"#;
