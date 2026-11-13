(impl_item
  type: [(type_identifier) @owner (generic_type type: (type_identifier) @owner) (scoped_type_identifier name: (type_identifier) @owner)]
  body: (declaration_list (function_item name: (identifier) @name) @def.method))
(trait_item body: (declaration_list (function_item name: (identifier) @name) @def.method))
(trait_item body: (declaration_list (function_signature_item name: (identifier) @name) @def.method))
(function_item name: (identifier) @name) @def.function
(struct_item name: (type_identifier) @name) @def.struct
(enum_item name: (type_identifier) @name) @def.enum
(union_item name: (type_identifier) @name) @def.struct
(type_item name: (type_identifier) @name) @def.type
(trait_item name: (type_identifier) @name) @def.trait
(mod_item name: (identifier) @name body: (declaration_list)) @def.module

(call_expression function: (identifier) @callee) @call
(call_expression function: (field_expression value: (_) @receiver field: (field_identifier) @callee)) @call
(call_expression function: (scoped_identifier path: (_) @receiver name: (identifier) @callee)) @call
(call_expression function: (generic_function function: (identifier) @callee)) @call
(call_expression function: (generic_function function: (scoped_identifier path: (_) @receiver name: (identifier) @callee))) @call
(call_expression function: (generic_function function: (field_expression value: (_) @receiver field: (field_identifier) @callee))) @call
(struct_expression name: (type_identifier) @callee) @call


(impl_item trait: [(type_identifier) @base (scoped_type_identifier name: (type_identifier) @base) (generic_type type: (type_identifier) @base)] type: [(type_identifier) @owner (generic_type type: (type_identifier) @owner)]) @inherit
