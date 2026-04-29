/// Tree-sitter query for Elixir language constructs.
///
/// Captures: modules, functions, macros, protocols, structs.
pub const QUERY: &str = r#"
; Module definitions
(call
  target: (identifier) @name.definition.module
  (arguments
    (alias) @name.definition.module_name))
  (#eq? @name.definition.module "defmodule")

; Function definitions
(call
  target: (identifier) @name.definition.function
  (arguments
    (call
      target: (identifier) @name.definition.function_name)))
  (#match? @name.definition.function "^def(p)?$")

; Private function definitions
(call
  target: (identifier) @name.definition.private_function
  (arguments
    (call
      target: (identifier) @name.definition.private_function_name)))
  (#eq? @name.definition.private_function "defp")

; Macro definitions
(call
  target: (identifier) @name.definition.macro
  (arguments
    (call
      target: (identifier) @name.definition.macro_name)))
  (#match? @name.definition.macro "^defmacro(p)?$")

; Protocol definitions
(call
  target: (identifier) @name.definition.protocol
  (arguments
    (alias) @name.definition.protocol_name))
  (#eq? @name.definition.protocol "defprotocol")

; Implementation definitions
(call
  target: (identifier) @name.definition.implementation
  (arguments
    (alias) @name.definition.implementation_name))
  (#eq? @name.definition.implementation "defimpl")

; Struct definitions
(call
  target: (identifier) @name.definition.struct
  (arguments
    [
      (alias) @name.definition.struct_name
      (map
        (map_content)+)
    ]))
  (#eq? @name.definition.struct "defstruct")

; Module attributes
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @name.definition.attribute)) @definition.attribute

; Type specifications
(call
  target: (identifier) @name.definition.spec
  (arguments
    (call
      target: (operator
        left: (identifier) @name.definition.spec_function))))
  (#match? @name.definition.spec "^@spec")
"#;
