; Gorp highlights, using the standard tree-sitter capture names.
; When several patterns capture the same node, the LAST one wins (as in
; tree-sitter-highlight, Neovim and Helix), so general patterns come first.

(identifier) @variable

; Punctuation and operators
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":"] @punctuation.delimiter

[
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "+"
  "-"
  "*"
  "/"
  "%"
  "!"
  "?"
  "="
  "|>"
  "->"
  "=>"
] @operator
(wildcard) @operator

(type_parameters ["<" ">"] @punctuation.bracket)
(type_arguments ["<" ">"] @punctuation.bracket)

; Keywords
[
  "component"
  "type"
  "enum"
  "import"
  "forbid"
  "module"
  "project"
  "entry"
  "guidelines"
  "extend"
  "replace"
  "uses"
  "state"
  "property"
  "example"
  "invariant"
  "given"
  "let"
  "for"
  "when"
  "where"
] @keyword

[
  "and"
  "or"
  "not"
  "in"
] @keyword.operator

; Literals
(integer) @number
(float) @number
(boolean) @constant.builtin
(none) @constant.builtin
(unit) @constant.builtin

; Capitalised names used as values (`Open`, `!NotFound(id)`) are variants.
(type_identifier) @constructor

; Types
(type_definition name: (type_identifier) @type)
(enum_definition name: (type_identifier) @type)
(component name: (type_identifier) @type)
(named_type (type_identifier) @type)
(type_path (type_identifier) @type)
(type_parameters (type_identifier) @type)
(record_expression type: (type_identifier) @type)
(path (type_identifier) @type)
(wildcard_path (type_identifier) @type)
(import name: (type_identifier) @type)
(variant name: (type_identifier) @constructor)

; Builtin names
((identifier) @variable.builtin
  (#any-of? @variable.builtin "it" "state"))

; Modules
(import module: (identifier) @module)
(module name: (identifier) @module)
(module_declaration name: (identifier) @module)
(project name: (identifier) @module)
(guideline topic: (identifier) @label)

; Fields
(field_definition name: (_) @property)
(field_initializer name: (identifier) @property)
(field_expression field: (identifier) @property)

; Functions and methods
(method name: (identifier) @function.method)
(call_expression function: (identifier) @function)
(call_expression
  function: (field_expression field: (identifier) @function.method))
(pipe_expression function: (identifier) @function)

; Parameters and bindings
(parameter name: (identifier) @variable.parameter)
(lambda parameters: (identifier) @variable.parameter)
(lambda_parameters (identifier) @variable.parameter)
(let_statement name: (identifier) @variable)

; Strings
(string) @string
(escape_sequence) @string.escape
(interpolation) @embedded
(interpolation ["${" "}"] @punctuation.special)

; Intents: `~` and `~"..."` stand apart from ordinary strings.
(intent) @string.special
(intent text: (intent_text) @string.special)

; Holes and comments
(hole) @string.special
(comment) @comment
