; ripwire Scala tags
(function_definition
  name: (identifier) @name) @definition.function


(function_declaration
  name: (identifier) @name) @definition.function
(class_definition
  name: (identifier) @name) @definition.class

(trait_definition
  name: (identifier) @name) @definition.interface

(object_definition
  name: (identifier) @name) @definition.class

(call_expression
  function: (identifier) @name) @reference.call


(call_expression
  function: (field_expression
    field: (identifier) @name)) @reference.call
(extends_clause
  type: (type_identifier) @name) @reference.extends
