/// Tree-sitter query for Vue single-file components.
///
/// Captures: template, script, style sections, component definitions.
pub const QUERY: &str = r#"
; Template section
(template_element) @definition.template

; Script section
(script_element) @definition.script

; Style section
(style_element) @definition.style

; Component definitions in script
(call_expression
  function: (identifier) @func_name
  arguments: (arguments
    (object
      (pair
        key: (property_identifier) @prop_name
        value: (string) @name))))
  (#match? @func_name "^(defineComponent|extend)$")
  (#eq? @prop_name "name")

; Export default component
(export_statement
  (default
    (call_expression
      function: (identifier) @func_name)))
  (#match? @func_name "^(defineComponent|extend)$")

; Vue composition API setup
(call_expression
  function: (identifier) @func_name)
  (#match? @func_name "^(ref|reactive|computed|watch|onMounted|onUnmounted)$")
"#;
