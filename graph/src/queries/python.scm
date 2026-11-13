(function_definition name: (identifier) @name) @def.function
(class_definition name: (identifier) @name) @def.class

(call function: (identifier) @callee) @call
(call function: (attribute object: (_) @receiver attribute: (identifier) @callee)) @call

(import_from_statement module_name: [(dotted_name) (relative_import)] @source name: (dotted_name) @imported) @import
(import_from_statement module_name: [(dotted_name) (relative_import)] @source name: (aliased_import name: (dotted_name) @imported alias: (identifier) @alias)) @import
(import_statement name: (dotted_name) @source) @import
(import_statement name: (aliased_import name: (dotted_name) @source alias: (identifier) @alias)) @import

(class_definition superclasses: (argument_list (identifier) @base)) @inherit
(class_definition superclasses: (argument_list (attribute attribute: (identifier) @base))) @inherit
