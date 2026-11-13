(function_definition name: (name) @name) @def.function
(method_declaration name: (name) @name) @def.method
(class_declaration name: (name) @name) @def.class
(interface_declaration name: (name) @name) @def.interface
(trait_declaration name: (name) @name) @def.trait
(enum_declaration name: (name) @name) @def.enum

(function_call_expression function: (name) @callee) @call
(function_call_expression function: (qualified_name (name) @callee .)) @call
(member_call_expression object: (_) @receiver name: (name) @callee) @call
(nullsafe_member_call_expression object: (_) @receiver name: (name) @callee) @call
(scoped_call_expression scope: (_) @receiver name: (name) @callee) @call
(object_creation_expression (name) @callee) @call
(object_creation_expression (qualified_name (name) @callee .)) @call

(namespace_use_declaration (namespace_use_clause (qualified_name) @source)) @import
(namespace_use_declaration (namespace_name) @source body: (namespace_use_group (namespace_use_clause (name) @imported))) @import

(class_declaration (base_clause (name) @base)) @inherit
(class_declaration (base_clause (qualified_name (name) @base .))) @inherit
(class_declaration (class_interface_clause (name) @base)) @inherit
(class_declaration (class_interface_clause (qualified_name (name) @base .))) @inherit
