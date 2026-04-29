/// Tree-sitter query for Solidity language constructs.
///
/// Captures: contracts, interfaces, libraries, functions, events, modifiers.
pub const QUERY: &str = r#"
; Contract definitions
(contract_declaration
  name: (identifier) @name.definition.contract) @definition.contract

; Abstract contract definitions
(contract_declaration
  (abstract_modifier)
  name: (identifier) @name.definition.abstract_contract) @definition.abstract_contract

; Interface definitions
(interface_declaration
  name: (identifier) @name.definition.interface) @definition.interface

; Library definitions
(library_declaration
  name: (identifier) @name.definition.library) @definition.library

; Function definitions
(function_definition
  name: (function_name) @name.definition.function) @definition.function

; Constructor definitions
(constructor_definition) @definition.constructor

; Modifier definitions
(modifier_definition
  name: (identifier) @name.definition.modifier) @definition.modifier

; Event definitions
(event_definition
  name: (identifier) @name.definition.event) @definition.event

; Error definitions
(error_definition
  name: (identifier) @name.definition.error) @definition.error

; Struct definitions
(struct_definition
  name: (identifier) @name.definition.struct) @definition.struct

; Enum definitions
(enum_definition
  name: (identifier) @name.definition.enum) @definition.enum

; State variable declarations
(state_variable_declaration
  name: (identifier) @name.definition.variable) @definition.variable

; Using declarations
(using_directive) @definition.using

; Pragma declarations
(pragma_directive) @definition.pragma

; Import directives
(import_directive) @definition.import
"#;
