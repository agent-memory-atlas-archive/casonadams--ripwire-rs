; ripwire Erlang tags
(function_clause
  name: (atom) @name) @definition.function

(call
  expr: (atom) @name) @reference.call

(call
  expr: (remote
    fun: (atom) @name)) @reference.call
