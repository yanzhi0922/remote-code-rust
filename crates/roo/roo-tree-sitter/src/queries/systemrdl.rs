/// Tree-sitter query for SystemRDL language constructs.
///
/// Captures: components, properties, signals, fields, registers.
pub const QUERY: &str = r#"
; Component definitions (addrmap, regfile, reg, field, signal, etc.)
(component_definition
  type: (component_type) @name.definition.component_type
  name: (identifier) @name.definition.component) @definition.component

; Property definitions
(property_definition
  name: (identifier) @name.definition.property) @definition.property

; Enum definitions
(enum_definition
  name: (identifier) @name.definition.enum) @definition.enum

; Struct definitions
(struct_definition
  name: (identifier) @name.definition.struct) @definition.struct

; Constraint definitions
(constraint_definition
  name: (identifier) @name.definition.constraint) @definition.constraint

; Signal declarations
(signal_declaration
  name: (identifier) @name.definition.signal) @definition.signal

; Field declarations
(field_declaration
  name: (identifier) @name.definition.field) @definition.field

; Register declarations
(register_declaration
  name: (identifier) @name.definition.register) @definition.register

; Instance declarations
(instance_declaration
  name: (identifier) @name.definition.instance) @definition.instance

; Alias declarations
(alias_declaration
  name: (identifier) @name.definition.alias) @definition.alias

; Dynamic property assignments
(dynamic_property_assignment) @definition.property_assignment
"#;
