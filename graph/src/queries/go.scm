(function_declaration name: (identifier) @name) @def.function
(method_declaration
  receiver: (parameter_list (parameter_declaration type: [(type_identifier) @owner (pointer_type (type_identifier) @owner) (generic_type type: (type_identifier) @owner) (pointer_type (generic_type type: (type_identifier) @owner))]))
  name: (field_identifier) @name) @def.method
(type_spec name: (type_identifier) @name type: (struct_type)) @def.struct
(type_spec name: (type_identifier) @name type: (interface_type)) @def.interface

(call_expression function: (identifier) @callee) @call
(call_expression function: (selector_expression operand: (_) @receiver field: (field_identifier) @callee)) @call

(import_spec name: (package_identifier) @alias path: (interpreted_string_literal) @source) @import
(import_spec !name path: (interpreted_string_literal) @source) @import

(type_spec type: (struct_type (field_declaration_list (field_declaration !name type: [(type_identifier) @base (pointer_type (type_identifier) @base)])))) @inherit
