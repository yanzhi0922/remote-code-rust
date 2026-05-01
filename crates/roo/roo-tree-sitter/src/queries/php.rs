/// Tree-sitter query for PHP language constructs.
///
/// Captures: classes, interfaces, traits, enums, functions, methods, properties.
pub const QUERY: &str = r#"
;--------------------------
; 1. CLASS DEFINITIONS
;--------------------------
; Regular classes
(class_declaration
  name: (name) @name.definition.class) @definition.class

; Abstract classes
(class_declaration
  (abstract_modifier)
  name: (name) @name.definition.abstract_class) @definition.abstract_class

; Final classes
(class_declaration
  (final_modifier)
  name: (name) @name.definition.final_class) @definition.final_class

; Readonly classes (PHP 8.2+)
(class_declaration
  (readonly_modifier)
  name: (name) @name.definition.readonly_class) @definition.readonly_class

;--------------------------
; 2. INTERFACE & TRAIT DEFINITIONS
;--------------------------
; Interfaces
(interface_declaration
  name: (name) @name.definition.interface) @definition.interface

; Traits
(trait_declaration
  name: (name) @name.definition.trait) @definition.trait

; Enums (PHP 8.1+)
(enum_declaration
  name: (name) @name.definition.enum) @definition.enum

;--------------------------
; 3. FUNCTION & METHOD DEFINITIONS
;--------------------------
; Function definitions
(function_definition
  name: (name) @name.definition.function) @definition.function

; Method definitions
(method_declaration
  name: (name) @name.definition.method) @definition.method

; Abstract methods
(method_declaration
  (abstract_modifier)
  name: (name) @name.definition.abstract_method) @definition.abstract_method

; Final methods
(method_declaration
  (final_modifier)
  name: (name) @name.definition.final_method) @definition.final_method

; Static methods
(method_declaration
  (static_modifier)
  name: (name) @name.definition.static_method) @definition.static_method

; Arrow functions
(arrow_function) @definition.arrow_function

;--------------------------
; 4. PROPERTY DEFINITIONS
;--------------------------
; Regular properties
(property_declaration
  (property_element
    (variable_name
      (name) @name.definition.property))) @definition.property

; Static properties
(property_declaration
  (static_modifier)
  (property_element
    (variable_name
      (name) @name.definition.static_property))) @definition.static_property

; Readonly properties (PHP 8.1+)
(property_declaration
  (readonly_modifier)
  (property_element
    (variable_name
      (name) @name.definition.readonly_property))) @definition.readonly_property

;--------------------------
; 5. OTHER LANGUAGE CONSTRUCTS
;--------------------------
; Constants
(const_declaration
  (const_element
    (name) @name.definition.constant))) @definition.constant

; Namespaces
(namespace_definition
  (namespace_name
    (name) @name.definition.namespace)) @definition.namespace

; Use statements (imports)
(use_declaration) @definition.use

; Anonymous classes
(anonymous_function) @definition.anonymous_function

; Match expressions (PHP 8.0+)
(match_expression) @definition.match

; Attributes (PHP 8.0+)
(attribute) @definition.attribute
"#;
