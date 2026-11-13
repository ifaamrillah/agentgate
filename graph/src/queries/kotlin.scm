(class_body (function_declaration name: (identifier) @name) @def.method)
(function_declaration name: (identifier) @name) @def.function
(class_declaration name: (identifier) @name) @def.class
(object_declaration name: (identifier) @name) @def.class

(call_expression . (identifier) @callee) @call
(call_expression . (navigation_expression . (_) @receiver (identifier) @callee .)) @call

(import (qualified_identifier) @source) @import

(class_declaration (delegation_specifiers (delegation_specifier (user_type (identifier) @base)))) @inherit
(class_declaration (delegation_specifiers (delegation_specifier (constructor_invocation (user_type (identifier) @base))))) @inherit
