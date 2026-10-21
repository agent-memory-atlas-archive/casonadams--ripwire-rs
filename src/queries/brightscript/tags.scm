; ripwire-rs BrightScript & BrighterScript tags
;
; ---- definitions ----

; Function declaration: function foo(...) ... end function
(function_statement
  name: (identifier) @name) @definition.function

; Sub declaration: sub bar(...) ... end sub
(sub_statement
  name: (identifier) @name) @definition.function

; ---- calls ----

; Direct call: foo(...)
(function_call
  function: (prefix_exp
    (identifier) @name)) @reference.call

; Member / method call: obj.method(...) or m.method(...)
(function_call
  function: (prefix_exp
    (prefix_exp)
    (identifier) @name)) @reference.call

; ---- imports ----

; Library import: library "v30/bslCore.brs"
(library_statement
  path: (string) @import.path)
