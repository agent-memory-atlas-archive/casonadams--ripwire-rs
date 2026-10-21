; ripwire-rs Zig tags
(function_declaration
  (identifier) @name) @definition.function

(variable_declaration
  (identifier) @name
  (struct_declaration)) @definition.class

(variable_declaration
  (identifier) @name
  (enum_declaration)) @definition.type

(variable_declaration
  (identifier) @name
  (union_declaration)) @definition.class

(call_expression
  (identifier) @name) @reference.call

(call_expression
  function: (field_expression
    member: (identifier) @name)) @reference.call
