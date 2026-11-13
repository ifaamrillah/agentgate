; TypeScript/TSX: seperti JavaScript, nama kelas/interface berupa type_identifier.
(function_declaration name: (identifier) @name) @def.function
(generator_function_declaration name: (identifier) @name) @def.function
(function_signature name: (identifier) @name) @def.function
(class_declaration name: (type_identifier) @name) @def.class
(abstract_class_declaration name: (type_identifier) @name) @def.class
(interface_declaration name: (type_identifier) @name) @def.interface
(type_alias_declaration name: (type_identifier) @name) @def.type
(enum_declaration name: (identifier) @name) @def.enum
(internal_module name: (identifier) @name) @def.module
(method_definition name: [(property_identifier) (private_property_identifier)] @name) @def.method
(method_signature name: (property_identifier) @name) @def.method
(abstract_method_signature name: (property_identifier) @name) @def.method
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

(class_declaration (class_heritage (extends_clause value: (identifier) @base))) @inherit
(class_declaration (class_heritage (implements_clause (type_identifier) @base))) @inherit
(abstract_class_declaration (class_heritage (extends_clause value: (identifier) @base))) @inherit
(abstract_class_declaration (class_heritage (implements_clause (type_identifier) @base))) @inherit
(interface_declaration (extends_type_clause type: (type_identifier) @base)) @inherit
