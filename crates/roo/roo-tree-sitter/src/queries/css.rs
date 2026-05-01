/// Tree-sitter query for CSS language constructs.
///
/// Captures: rulesets, selectors, declarations, media queries, keyframes.
pub const QUERY: &str = r#"
; Rulesets
(rule_set
  (selectors) @name.definition.selector) @definition.ruleset

; Media queries
(media_statement) @definition.media

; Keyframes
(keyframes_statement
  (identifier) @name.definition.keyframes) @definition.keyframes

; Supports queries
(supports_statement) @definition.supports

; Import statements
(import_statement) @definition.import

; Namespace statements
(namespace_statement) @definition.namespace

; Font-face
(font_face_statement) @definition.font_face

; Custom properties (CSS variables)
(declaration
  (property_name
    (custom_property_name) @name.definition.custom_property)) @definition.custom_property

; ID selectors
(id_selector
  (id_name) @name.definition.id) @definition.id_selector

; Class selectors
(class_selector
  (class_name) @name.definition.class) @definition.class_selector

; Pseudo-class selectors
(pseudo_class_selector
  (class_name) @name.definition.pseudo_class)) @definition.pseudo_class

; Pseudo-element selectors
(pseudo_element_selector
  (tag_name) @name.definition.pseudo_element)) @definition.pseudo_element

; Attribute selectors
(attribute_selector
  (attribute_name) @name.definition.attribute)) @definition.attribute_selector

; Nesting
(nested_rule_set) @definition.nested_ruleset
"#;
