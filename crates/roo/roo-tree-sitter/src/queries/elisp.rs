/// Tree-sitter query for Emacs Lisp language constructs.
///
/// Captures: defun, defmacro, defvar, defconst, defstruct, etc.
pub const QUERY: &str = r#"
; Function definitions
(function_definition
  name: (symbol) @name.definition.function) @definition.function

; Macro definitions
(macro_definition
  name: (symbol) @name.definition.macro) @definition.macro

; Variable definitions
(variable_definition
  name: (symbol) @name.definition.variable) @definition.variable

; Constant definitions
(constant_definition
  name: (symbol) @name.definition.constant) @definition.constant

; Struct definitions
(struct_definition
  name: (symbol) @name.definition.struct) @definition.struct

; Class definitions
(class_definition
  name: (symbol) @name.definition.class) @definition.class

; Generic function definitions
(generic_function_definition
  name: (symbol) @name.definition.generic) @definition.generic

; Feature provides
(provide_statement
  (symbol) @name.definition.feature) @definition.feature

; Require statements
(require_statement
  (symbol) @name.definition.require) @definition.require

; Defalias
(alias_definition
  name: (symbol) @name.definition.alias) @definition.alias
"#;
