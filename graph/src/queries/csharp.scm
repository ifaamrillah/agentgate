(method_declaration name: (identifier) @name) @def.method
(constructor_declaration name: (identifier) @name) @def.method
(local_function_statement name: (identifier) @name) @def.function
(class_declaration name: (identifier) @name) @def.class
(record_declaration name: (identifier) @name) @def.class
(struct_declaration name: (identifier) @name) @def.struct
(interface_declaration name: (identifier) @name) @def.interface
(enum_declaration name: (identifier) @name) @def.enum

(invocation_expression function: (identifier) @callee) @call
(invocation_expression function: (member_access_expression expression: (_) @receiver name: (identifier) @callee)) @call
(invocation_expression function: (member_access_expression expression: _ @receiver name: (identifier) @callee)) @call
(invocation_expression function: (generic_name (identifier) @callee)) @call
(object_creation_expression type: (identifier) @callee) @call

(using_directive [(qualified_name) (identifier)] @source) @import

(class_declaration (base_list (identifier) @base)) @inherit
(class_declaration (base_list (generic_name (identifier) @base))) @inherit
(interface_declaration (base_list (identifier) @base)) @inherit
(struct_declaration (base_list (identifier) @base)) @inherit
(record_declaration (base_list (identifier) @base)) @inherit
