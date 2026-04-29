/// Tree-sitter query for Ruby language constructs.
///
/// Captures: methods, classes, modules, constants, blocks, procs, lambdas, etc.
pub const QUERY: &str = r#"
; Method definitions
(method
  name: (identifier) @name.definition.method) @definition.method

; Singleton methods
(singleton_method
  object: (_)
  name: (identifier) @name.definition.method) @definition.method

; Method aliases
(alias
  name: (_) @name.definition.method) @definition.method

; Class definitions
(class
  name: [
    (constant) @name.definition.class
    (scope_resolution
      name: (_) @name.definition.class)
  ]) @definition.class

; Singleton classes
(singleton_class
  value: [
    (constant) @name.definition.class
    (scope_resolution
      name: (_) @name.definition.class)
  ]) @definition.class

; Module definitions
(module
  name: [
    (constant) @name.definition.module
    (scope_resolution
      name: (_) @name.definition.module)
  ]) @definition.module

; Constants
(assignment
  left: (constant) @name.definition.constant) @definition.constant

; Global variables
(global_variable) @definition.global_variable

; Instance variables
(instance_variable) @definition.instance_variable

; Class variables
(class_variable) @definition.class_variable

; Symbols
(simple_symbol) @definition.symbol
(hash_key_symbol) @definition.symbol

; Blocks
(block) @definition.block

; Procs
(block
  body: (block_body)) @definition.proc

; Lambdas
(lambda) @definition.lambda

; Mixins - include
(call
  method: (identifier) @name.definition.mixin
  arguments: (argument_list
    (constant) @name.definition.mixin_target))
  (#match? @name.definition.mixin "^(include|extend|prepend)$")

; Attribute accessors
(call
  method: (identifier) @name.definition.accessor
  arguments: (argument_list
    (symbol) @name.definition.accessor_name))
  (#match? @name.definition.accessor "^(attr_reader|attr_writer|attr_accessor)$")

; Exception handling
(begin) @definition.begin
(rescue) @definition.rescue

; Pattern matching
(case
  (in_clause)) @definition.pattern_match

; Endless methods (Ruby 3.0+)
(method
  name: (identifier) @name.definition.endless_method) @definition.endless_method
"#;
