/// Tree-sitter query for embedded template languages (EJS, ERB).
///
/// Captures: template directives, output tags, comments.
pub const QUERY: &str = r#"
; Template directives
(directive) @definition.directive

; Output tags
(output) @definition.output

; Comment tags
(comment) @definition.comment

; Code sections
(code) @definition.code

; Control flow - if
(control_flow
  keyword: "if" @name.definition.if) @definition.if

; Control flow - else
(control_flow
  keyword: "else" @name.definition.else) @definition.else

; Control flow - for
(control_flow
  keyword: "for" @name.definition.for) @definition.for

; Control flow - while
(control_flow
  keyword: "while" @name.definition.while) @definition.while

; Control flow - unless
(control_flow
  keyword: "unless" @name.definition.unless) @definition.unless

; Block definitions
(block
  name: (identifier) @name.definition.block) @definition.block

; Partial includes
(partial
  name: (identifier) @name.definition.partial) @definition.partial
"#;
