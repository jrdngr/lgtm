//! Symbol queries, and highlight queries the grammar crates don't ship in our capture names.

pub(super) const RUST_SYMBOLS: &str = r#"
(function_item name: (identifier) @name) @symbol
(struct_item name: (type_identifier) @name) @symbol
(enum_item name: (type_identifier) @name) @symbol
(union_item name: (type_identifier) @name) @symbol
(trait_item name: (type_identifier) @name) @symbol
(impl_item type: [
  (type_identifier) @name
  (scoped_type_identifier name: (type_identifier) @name)
  (generic_type type: [
    (type_identifier) @name
    (scoped_type_identifier name: (type_identifier) @name)
  ])
  (reference_type) @name
  (pointer_type) @name
  (tuple_type) @name
  (unit_type) @name
  (array_type) @name
  (function_type) @name
  (dynamic_type) @name
  (primitive_type) @name
  (bounded_type) @name
  (never_type) @name
  (macro_invocation) @name
]) @symbol
(mod_item name: (identifier) @name) @symbol
(macro_definition name: (identifier) @name) @symbol
"#;

pub(super) const PYTHON_SYMBOLS: &str = r#"
(function_definition name: (identifier) @name) @symbol
(class_definition name: (identifier) @name) @symbol
"#;

pub(super) const JS_SYMBOLS: &str = r#"
(function_declaration name: (identifier) @name) @symbol
(generator_function_declaration name: (identifier) @name) @symbol
(class_declaration name: (_) @name) @symbol
(method_definition name: (_) @name) @symbol
(lexical_declaration
  (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)])) @symbol
"#;

pub(super) const TS_SYMBOLS: &str = r#"
(interface_declaration name: (_) @name) @symbol
(abstract_class_declaration name: (_) @name) @symbol
(enum_declaration name: (_) @name) @symbol
(type_alias_declaration name: (_) @name) @symbol
(internal_module name: (_) @name) @symbol
"#;

pub(super) const GO_SYMBOLS: &str = r#"
(function_declaration name: (identifier) @name) @symbol
(method_declaration name: (field_identifier) @name) @symbol
((type_declaration [
  (type_spec name: (type_identifier) @name)
  (type_alias name: (type_identifier) @name)
]) @symbol
  (#not-match? @symbol "^type\\s*\\("))
(type_declaration "(" [
  (type_spec name: (type_identifier) @name)
  (type_alias name: (type_identifier) @name)
] @symbol)
"#;

pub(super) const SWIFT_SYMBOLS: &str = r#"
(class_declaration name: (_) @name) @symbol
(protocol_declaration name: (_) @name) @symbol
(function_declaration name: (_) @name) @symbol
(protocol_function_declaration name: (_) @name) @symbol
(init_declaration "init" @name) @symbol
(deinit_declaration "deinit" @name) @symbol
(subscript_declaration "subscript" @name) @symbol
"#;

/// The crate's queries use nvim's capture names (`text.title`), which match none of ours.
pub(super) const MD_BLOCK_HIGHLIGHTS: &str = r#"
(atx_heading (inline) @tag)
(setext_heading (paragraph) @tag)
[
  (atx_h1_marker) (atx_h2_marker) (atx_h3_marker) (atx_h4_marker) (atx_h5_marker)
  (atx_h6_marker) (setext_h1_underline) (setext_h2_underline) (thematic_break)
  (block_quote_marker) (fenced_code_block_delimiter)
] @punctuation
[
  (list_marker_plus) (list_marker_minus) (list_marker_star) (list_marker_dot)
  (list_marker_parenthesis)
] @operator
(info_string) @label
(indented_code_block) @string
[(link_destination) (link_title)] @string
(link_label) @label
(backslash_escape) @escape
"#;

pub(super) const MD_INLINE_HIGHLIGHTS: &str = r#"
(code_span) @string
[(emphasis_delimiter) (code_span_delimiter)] @punctuation
(emphasis) @variable.parameter
(strong_emphasis) @keyword
[(link_destination) (uri_autolink) (link_title)] @string
[(link_text) (image_description)] @function
(link_label) @label
[(backslash_escape) (hard_line_break)] @escape
"#;

/// The crate's injections, but with `include-children` on inline text, without which the
/// inline grammar never highlights anything.
pub(super) const MD_INJECTIONS: &str = r#"
(fenced_code_block
  (info_string (language) @injection.language)
  (code_fence_content) @injection.content)
((minus_metadata) @injection.content (#set! injection.language "yaml"))
((plus_metadata) @injection.content (#set! injection.language "toml"))
((inline) @injection.content
  (#set! injection.language "markdown_inline")
  (#set! injection.include-children))
"#;

/// The crate ships no queries. Earlier patterns win where two match the same node.
pub(super) const HCL_HIGHLIGHTS: &str = r#"
(comment) @comment
(block . (identifier) @type)
(attribute . (identifier) @property)
(object_elem key: (expression (variable_expr (identifier) @property)))
(get_attr (identifier) @property)
(function_call . (identifier) @function)
((variable_expr (identifier) @variable.builtin)
  (#any-of? @variable.builtin
    "var" "local" "data" "module" "each" "count" "self" "path" "terraform"))
(for_intro (identifier) @variable.parameter)
(template_for_start (identifier) @variable.parameter)
[(string_lit) (template_literal) (quoted_template_start) (quoted_template_end)] @string
(heredoc_template (template_literal) @string)
[(heredoc_start) (heredoc_identifier)] @punctuation
(numeric_lit) @number
[(bool_lit) (null_lit)] @constant
["if" "else" "endif" "for" "endfor" "in"] @keyword
[
  (template_interpolation_start) (template_interpolation_end)
  (template_directive_start) (template_directive_end) (strip_marker)
] @escape
[
  "!" "*" "/" "%" "+" "-" ">" ">=" "<" "<=" "==" "!=" "&&" "||" "?" ":" "=" "=>"
  (ellipsis)
] @operator
["." ".*" "[*]" "," "{" "}" "[" "]" "(" ")"] @punctuation
"#;

pub(super) const SHELL_SYMBOLS: &str = r#"
(function_definition name: (word) @name) @symbol
"#;

/// Sections, keyed by their heading. A section runs to the next heading of its level or above.
/// The grammar only opens sections at ATX headings, so a setext heading names a section only
/// when it starts one, like a document's leading title.
pub(super) const MD_SYMBOLS: &str = r#"
(section (atx_heading heading_content: (inline) @name)) @symbol
(section . (setext_heading heading_content: (paragraph (inline) @name))) @symbol
"#;

/// Keys holding an object or array. Keys with plain values would bury the outline.
pub(super) const JSON_SYMBOLS: &str = r#"
(pair key: (string (string_content) @name) value: [(object) (array)]) @symbol
"#;

pub(super) const YAML_SYMBOLS: &str = r#"
(block_mapping_pair key: (_) @name value: (block_node)) @symbol
"#;

pub(super) const TOML_SYMBOLS: &str = r#"
(table [(bare_key) (dotted_key) (quoted_key)] @name) @symbol
(table_array_element [(bare_key) (dotted_key) (quoted_key)] @name) @symbol
"#;

pub(super) const CSS_SYMBOLS: &str = r#"
(rule_set (selectors) @name) @symbol
(keyframes_statement (keyframes_name) @name) @symbol
"#;

pub(super) const SCSS_SYMBOLS: &str = r#"
(rule_set (selectors) @name) @symbol
(keyframes_statement (keyframes_name) @name) @symbol
(mixin_statement name: (identifier) @name) @symbol
(function_statement name: (identifier) @name) @symbol
"#;

/// Elements with an `id`, named by it, and script and style blocks.
pub(super) const HTML_SYMBOLS: &str = r#"
(element
  (start_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value (attribute_value) @name)))
  (#eq? @_attr "id")) @symbol
(script_element (start_tag (tag_name) @name)) @symbol
(style_element (start_tag (tag_name) @name)) @symbol
"#;

pub(super) const SQL_SYMBOLS: &str = r#"
(create_table (object_reference name: (identifier) @name)) @symbol
(create_view (object_reference name: (identifier) @name)) @symbol
(create_materialized_view (object_reference name: (identifier) @name)) @symbol
(create_function (object_reference name: (identifier) @name)) @symbol
"#;

pub(super) const MAKE_SYMBOLS: &str = r#"
(rule (targets) @name) @symbol
(define_directive name: (word) @name) @symbol
"#;

pub(super) const GORP_HIGHLIGHTS: &str = include_str!("../../grammars/gorp/queries/highlights.scm");

pub(super) const GORP_SYMBOLS: &str = r#"
(module name: (identifier) @name) @symbol
(type_definition name: (type_identifier) @name) @symbol
(enum_definition name: (type_identifier) @name) @symbol
(component name: (type_identifier) @name) @symbol
(method name: (identifier) @name) @symbol
"#;
