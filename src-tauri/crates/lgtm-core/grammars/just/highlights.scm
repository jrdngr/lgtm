; Base: identifiers are variable references; more specific captures below
; override this.
(identifier) @variable

(comment) @comment

; Strings and friends -------------------------------------------------------

(string) @string

(format_string) @string

(escape_sequence) @string.escape

(backtick
  (command_content) @none)

[
  "`"
  "```"
] @punctuation.special

(boolean) @boolean

; Definitions ----------------------------------------------------------------

(recipe
  name: (identifier) @function)

(alias
  name: (identifier) @function)

(alias
  target: (namepath
    (identifier) @function))

(assignment
  name: (identifier) @variable)

(unexport
  name: (identifier) @variable)

(function_definition
  name: (identifier) @function)

(parameter
  name: (identifier) @variable.parameter)

(module
  name: (identifier) @module)

(setting
  name: (identifier) @property)

(attribute
  name: (identifier) @attribute)

(keyword_argument
  name: (identifier) @variable.parameter)

; Calls and dependencies ------------------------------------------------------

(function_call
  name: (identifier) @function.call)

(dependency
  name: (namepath
    (identifier) @function.call))

; Recipe bodies ---------------------------------------------------------------

(shebang) @keyword.directive

(shebang
  (language) @label)

(line_prefix) @operator

; Keywords --------------------------------------------------------------------

[
  "alias"
  "eager"
  "export"
  "import"
  "mod"
  "set"
  "unexport"
] @keyword

[
  "if"
  "else"
] @keyword.conditional

"assert" @keyword

; Operators and punctuation ---------------------------------------------------

[
  ":="
  "=="
  "!="
  "=~"
  "!~"
  "&&"
  "||"
  "+"
  "++"
  "/"
  "!"
  "="
  "*"
  "$"
  "?"
  "@"
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "{{"
  "}}"
] @punctuation.special

[
  ","
  ":"
  "::"
] @punctuation.delimiter
