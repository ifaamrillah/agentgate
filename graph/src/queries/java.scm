(method_declaration name: (identifier) @name) @def.method
(constructor_declaration name: (identifier) @name) @def.method
(class_declaration name: (identifier) @name) @def.class
(record_declaration name: (identifier) @name) @def.class
(interface_declaration name: (identifier) @name) @def.interface
(enum_declaration name: (identifier) @name) @def.enum

(method_invocation object: (_) @receiver name: (identifier) @callee) @call
(method_invocation !object name: (identifier) @callee) @call
(object_creation_expression type: (type_identifier) @callee) @call

(import_declaration (scoped_identifier) @source) @import

(class_declaration superclass: (superclass (type_identifier) @base)) @inherit
(class_declaration superclass: (superclass (generic_type (type_identifier) @base))) @inherit
(class_declaration interfaces: (super_interfaces (type_list (type_identifier) @base))) @inherit
(class_declaration interfaces: (super_interfaces (type_list (generic_type (type_identifier) @base)))) @inherit
(interface_declaration (extends_interfaces (type_list (type_identifier) @base))) @inherit
