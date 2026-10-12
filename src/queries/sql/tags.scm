; ripwire SQL tags
(create_table
  (object_reference
    name: (identifier) @name)) @definition.class

(create_view
  (object_reference
    name: (identifier) @name)) @definition.class

(create_function
  (object_reference
    name: (identifier) @name)) @definition.function
