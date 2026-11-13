(class_declaration name: (identifier) @name) @def.class
(mixin_declaration (identifier) @name) @def.class
(extension_declaration name: (identifier) @name) @def.class
(enum_declaration name: (identifier) @name) @def.enum
(function_declaration signature: (function_signature name: (identifier) @name)) @def.function
(method_declaration signature: (method_signature (function_signature name: (identifier) @name))) @def.method
(member_declaration (declaration (function_signature name: (identifier) @name))) @def.method

(call_expression function: (identifier) @callee) @call
(call_expression function: (member_expression object: (_) @receiver property: (identifier) @callee)) @call
(call_expression function: (member_expression object: _ @receiver property: (identifier) @callee)) @call

(import_specification uri: (configurable_uri (uri (string_literal) @source))) @import
(import_specification uri: (configurable_uri (uri (string_literal) @source)) alias: (identifier) @alias) @import

(class_declaration superclass: (superclass type: (type (type_identifier) @base))) @inherit
(class_declaration superclass: (superclass (mixins (type (type_identifier) @base)))) @inherit
(class_declaration interfaces: (interfaces (type (type_identifier) @base))) @inherit
