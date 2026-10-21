; ripwire-rs COBOL tags

; ---- definitions ----

(identification_division
  (program_name) @name) @definition.module

(paragraph_header) @name @definition.function

(section_header) @name @definition.function

; ---- calls ----

(perform_procedure) @name @reference.call

(call_statement
  x: _ @name) @reference.call

; ---- imports ----

(copy_statement
  book: _ @import.path)
