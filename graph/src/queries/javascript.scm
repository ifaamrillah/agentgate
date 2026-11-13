; Definisi, call, import, dan pewarisan JavaScript (dipakai juga TypeScript lewat ts_common).
(function_declaration name: (identifier) @name) @def.function
(generator_function_declaration name: (identifier) @name) @def.function
(class_declaration name: (identifier) @name) @def.class
(method_definition name: [(property_identifier) (private_property_identifier)] @name) @def.method
(variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @def.function
(pair key: (property_identifier) @name value: [(arrow_function) (function_expression)]) @def.method
(assignment_expression
  left: (member_expression property: (property_identifier) @name)
  right: [(arrow_function) (function_expression)]) @def.function

(call_expression function: (identifier) @callee) @call
(call_expression function: (member_expression object: (_) @receiver property: [(property_identifier) (private_property_identifier)] @callee)) @call
(new_expression constructor: (identifier) @callee) @call

(import_statement (import_clause (named_imports (import_specifier name: (identifier) @imported))) source: (string) @source) @import
(import_statement (import_clause (identifier) @imported) source: (string) @source) @import
(import_statement (import_clause (namespace_import (identifier) @alias)) source: (string) @source) @import
(variable_declarator
  name: (identifier) @alias
  value: (call_expression function: (identifier) @_req arguments: (arguments (string) @source))
  (#eq? @_req "require")) @import
(variable_declarator
  name: (object_pattern (shorthand_property_identifier_pattern) @imported)
  value: (call_expression function: (identifier) @_req arguments: (arguments (string) @source))
  (#eq? @_req "require")) @import

(class_declaration name: (identifier) (class_heritage (identifier) @base)) @inherit
(class_declaration name: (identifier) (class_heritage (member_expression property: (property_identifier) @base))) @inherit
