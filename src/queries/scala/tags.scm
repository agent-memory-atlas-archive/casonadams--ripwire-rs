; ripwire Scala tags
(function_definition
  name: (identifier) @name) @definition.function

(class_definition
  name: (identifier) @name) @definition.class

(trait_definition
  name: (identifier) @name) @definition.interface

(object_definition
  name: (identifier) @name) @definition.class

(call_expression
  function: (identifier) @name) @reference.call
