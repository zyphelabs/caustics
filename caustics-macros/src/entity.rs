use crate::common::is_option;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{Data, DeriveInput, Fields};

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub is_optional: bool,
    pub is_primary_key: bool,
    pub is_created_at: bool,
    pub is_updated_at: bool,
    pub column_name: Option<String>,
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = format_ident!("{}", self.name);
        let ty = syn::parse_str::<syn::Type>(&self.ty).unwrap();
        tokens.extend(quote! { #name: #ty });
    }
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub name: String,
    pub target: syn::Path,
    pub kind: RelationKind,
    pub foreign_key_field: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RelationKind {
    HasMany,
    BelongsTo,
}

pub fn generate_entity(model_ast: DeriveInput, relation_ast: DeriveInput) -> TokenStream {
    // Extract fields
    let fields = match &model_ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named.named.iter().collect::<Vec<_>>(),
            _ => panic!("Expected named fields"),
        },
        _ => panic!("Expected a struct"),
    };

    // Extract relations from relation_ast
    let relations = extract_relations(&relation_ast);

    // Filter out primary key fields for set operations
    let primary_key_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    (meta.path.is_ident("sea_orm")
                        && meta.tokens.to_string().contains("primary_key"))
                        || meta.path.is_ident("primary_key")
                } else {
                    false
                }
            })
        })
        .collect();

    // Filter out unique fields (including primary keys)
    let unique_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    (meta.path.is_ident("sea_orm")
                        && (meta.tokens.to_string().contains("primary_key") || meta.tokens.to_string().contains("unique")))
                        || meta.path.is_ident("primary_key")
                        || meta.path.is_ident("unique")
                } else {
                    false
                }
            })
        })
        .collect();

    // Identify foreign key fields from relations
    let foreign_key_fields: Vec<_> = relations
        .iter()
        .filter_map(|relation| relation.foreign_key_field.clone())
        .collect();

    // Only non-nullable, non-primary-key, non-foreign-key fields are required
    let required_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            !primary_key_fields.contains(field) 
                && !is_option(&field.ty)
                && !foreign_key_fields.contains(&field_name)
        })
        .collect();

    // Generate struct fields for required fields (with pub)
    let required_struct_fields = required_fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            quote! { pub #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate function arguments for required fields (no pub)
    let required_fn_args = required_fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            quote! { #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate initializers for required fields (no pub)
    let required_inits = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate assignments for required fields (self.#name)
    let required_assigns = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { model.#name = sea_orm::ActiveValue::Set(self.#name); }
        })
        .collect::<Vec<_>>();

    // Generate foreign key relation fields for Create struct
    let foreign_key_relation_fields = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Check if the foreign key field is not nullable (not Option<T>)
                // Only required relations should be in the Create struct
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    !is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let relation_name = format_ident!("{}", relation.name.to_snake_case());
            let target_module = &relation.target;
            quote! {
                pub #relation_name: #target_module::UniqueWhereParam
            }
        })
        .collect::<Vec<_>>();

    // Generate foreign key relation function arguments
    let foreign_key_relation_args = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Check if the foreign key field is not nullable (not Option<T>)
                // Only required relations should be function arguments
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    !is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let relation_name = format_ident!("{}", relation.name.to_snake_case());
            let target_module = &relation.target;
            quote! {
                #relation_name: #target_module::UniqueWhereParam
            }
        })
        .collect::<Vec<_>>();

    // Generate foreign key relation initializers
    let foreign_key_relation_inits = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Check if the foreign key field is not nullable (not Option<T>)
                // Only required relations should be initializers
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    !is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let relation_name = format_ident!("{}", relation.name.to_snake_case());
            quote! { #relation_name }
        })
        .collect::<Vec<_>>();

    // Generate foreign key assignments (convert UniqueWhereParam to foreign key value)
    let foreign_key_assigns = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Check if the foreign key field is not nullable (not Option<T>)
                // Only required relations should be in foreign key assignments
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    !is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let fk_field = relation.foreign_key_field.as_ref().unwrap();
            let fk_field_ident = format_ident!("{}", fk_field);
            let relation_name = format_ident!("{}", relation.name.to_snake_case());
            let target_module = &relation.target;
            quote! {
                // Extract foreign key value from UniqueWhereParam
                let fk_value = match self.#relation_name {
                    #target_module::UniqueWhereParam::IdEquals(id) => id,
                    _ => panic!("Only IdEquals is supported for foreign key relations"),
                };
                model.#fk_field_ident = sea_orm::ActiveValue::Set(fk_value);
            }
        })
        .collect::<Vec<_>>();

    // Generate field variants for SetParam enum (excluding primary keys)
    let field_variants = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let ty = &field.ty;
            quote! {
                #pascal_name(sea_orm::ActiveValue<#ty>)
            }
        })
        .collect::<Vec<_>>();

    // Generate relation connection variants for SetParam enum
    let relation_connect_variants = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some()
        })
        .map(|relation| {
            let relation_name = format_ident!("Connect{}", relation.name.to_pascal_case());
            let target_module = &relation.target;
            let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
            
            // Check if this is an optional relation
            let is_optional = if let Some(field) = fields.iter().find(|f| {
                f.ident.as_ref().unwrap().to_string() == *fk_field_name
            }) {
                is_option(&field.ty)
            } else {
                false
            };
            
            if is_optional {
                // For optional relations, allow connecting to None or a specific entity
                quote! {
                    #relation_name(Option<#target_module::UniqueWhereParam>)
                }
            } else {
                // For required relations, only allow connecting to a specific entity
                quote! {
                    #relation_name(#target_module::UniqueWhereParam)
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation disconnect variants for SetParam enum
    let relation_disconnect_variants = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Only optional relations can be disconnected (set to None)
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let relation_name = format_ident!("Disconnect{}", relation.name.to_pascal_case());
            quote! {
                #relation_name
            }
        })
        .collect::<Vec<_>>();

    // Combine all SetParam variants
    let all_set_param_variants = quote! {
        #(#field_variants,)*
        #(#relation_connect_variants,)*
        #(#relation_disconnect_variants,)*
    };

    // Generate field variants for WhereParam enum (all fields)
    let where_field_variants = fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;
        quote! {
            #pascal_name(sea_orm::Condition)
        }
    }).collect::<Vec<_>>();

    // Generate field variants for OrderByParam enum (all fields)
    let order_by_field_variants = fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        quote! {
            #pascal_name(caustics::SortOrder)
        }
    }).collect::<Vec<_>>();

    // Generate match arms for WhereParam
    let where_match_arms = fields.iter().map(|field| {
        let pascal_name = format_ident!("{}", field.ident.as_ref().unwrap().to_string().to_pascal_case());
        quote! {
            WhereParam::#pascal_name(condition) => condition.clone()
        }
    }).collect::<Vec<_>>();

    // Generate match arms for UniqueWhereParam
    let unique_where_match_arms = unique_fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let equals_variant = format_ident!("{}Equals", pascal_name);
        quote! {
            UniqueWhereParam::#equals_variant(value) => {
                Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(value))
            }
        }
    }).collect::<Vec<_>>();

    // Generate match arms for OrderByParam
    let order_by_match_arms = fields.iter().map(|field| {
        let pascal_name = format_ident!("{}", field.ident.as_ref().unwrap().to_string().to_pascal_case());
        quote! {
            OrderByParam::#pascal_name(order) => {
                let sea_order = match order {
                    SortOrder::Asc => sea_orm::Order::Asc,
                    SortOrder::Desc => sea_orm::Order::Desc,
                };
                (<Entity as EntityTrait>::Column::#pascal_name, sea_order)
            }
        }
    }).collect::<Vec<_>>();

    // Generate UniqueWhereParam enum for unique fields
    let unique_where_variants = unique_fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = name.to_string().to_pascal_case();
        let equals_variant = format_ident!("{}Equals", pascal_name);
        let ty = &field.ty;
        quote! {
            #equals_variant(#ty)
        }
    }).collect::<Vec<_>>();

    // Generate UniqueWhereParam serialize implementation
    let unique_where_serialize_arms = unique_fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = name.to_string().to_pascal_case();
        let equals_variant = format_ident!("{}Equals", pascal_name);
        let field_name = name.to_string();
        quote! {
            UniqueWhereParam::#equals_variant(value) => (
                #field_name,
                ::prisma_client_rust::SerializedWhereValue::Value(
                    ::prisma_client_rust::PrismaValue::Int(value),
                ),
            ),
        }
    }).collect::<Vec<_>>();

    // Generate field operator modules (including primary keys for query operations)
    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let pascal_name = format_ident!("{}", field_name.as_ref().unwrap().to_string().to_pascal_case());
        let is_unique = unique_fields.iter().any(|unique_field| {
            unique_field.ident.as_ref().unwrap() == field_name.as_ref().unwrap()
        });

        let set_fn = if !is_unique {
            quote! {
                pub fn set<T: Into<#field_type>>(value: T) -> super::SetParam {
                    super::SetParam::#pascal_name(sea_orm::ActiveValue::Set(value.into()))
                }
            }
        } else {
            quote! {}
        };

        let unique_where_fn = if is_unique {
            let equals_variant = format_ident!("{}Equals", pascal_name);
            quote! {
                pub struct Equals(pub #field_type);
                
                pub fn equals<T: From<Equals>>(value: impl Into<#field_type>) -> T {
                    Equals(value.into()).into()
                }

                impl From<Equals> for #field_type {
                    fn from(Equals(v): Equals) -> Self {
                        v
                    }
                }
                
                impl From<Equals> for super::UniqueWhereParam {
                    fn from(Equals(v): Equals) -> Self {
                        super::UniqueWhereParam::#equals_variant(v)
                    }
                }
                
                impl From<Equals> for super::WhereParam {
                    fn from(Equals(v): Equals) -> Self {
                        super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.eq(v)))
                    }
                }
            }
        } else {
            quote! {
                pub fn equals<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.eq(value.into())))
                }
            }
        };

        let order_fn = quote! {
            pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
                super::OrderByParam::#pascal_name(order)
            }
        };
        quote! {
            pub mod #field_name {
                use sea_orm::{Condition, ColumnTrait, EntityTrait, ActiveValue};
                use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                use uuid::Uuid;
                use std::vec::Vec;

                #set_fn
                #unique_where_fn
                
                pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
                    super::OrderByParam::#pascal_name(order)
                }

     
                pub fn not_equals<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.ne(value.into())))
                }
                pub fn gt<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.gt(value.into())))
                }
                pub fn lt<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.lt(value.into())))
                }
                pub fn gte<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.gte(value.into())))
                }
                pub fn lte<T: Into<#field_type>>(value: T) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.lte(value.into())))
                }
                pub fn in_vec<T: Into<#field_type>>(values: Vec<T>) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.is_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>())))
                }
                pub fn not_in_vec<T: Into<#field_type>>(values: Vec<T>) -> super::WhereParam {
                    super::WhereParam::#pascal_name(Condition::all().add(<super::Entity as EntityTrait>::Column::#pascal_name.is_not_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>())))
                }
            }
        }
    });

    // Generate relation submodules
    let relation_submodules = generate_relation_submodules(&relations, &fields);

    // Generate ModelWithRelations struct fields
    let model_with_relations_fields = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            quote! { pub #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate field names for From implementation
    let field_names = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate field names and types for constructor
    let field_params = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            quote! { #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate relation fields for ModelWithRelations
    let relation_fields = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => quote! { pub #name: Option<Vec<#target::ModelWithRelations>> },
                RelationKind::BelongsTo => {
                    // Check if this is an optional relation by looking at the foreign key field
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields.iter().find(|f| {
                            f.ident.as_ref().unwrap().to_string() == *fk_field_name
                        }) {
                            is_option(&field.ty)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if is_optional {
                        // For optional relations: Option<Option<ModelWithRelations>>
                        // First Option: whether relation was fetched
                        // Second Option: whether relation exists in DB
                        quote! { pub #name: Option<Option<#target::ModelWithRelations>> }
                    } else {
                        // For required relations: Option<ModelWithRelations>
                        // Option: whether relation was fetched
                        quote! { pub #name: Option<#target::ModelWithRelations> }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation field names for constructor
    let relation_field_names = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => quote! { #name: Option<Vec<#target::ModelWithRelations>> },
                RelationKind::BelongsTo => {
                    // Check if this is an optional relation by looking at the foreign key field
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields.iter().find(|f| {
                            f.ident.as_ref().unwrap().to_string() == *fk_field_name
                        }) {
                            is_option(&field.ty)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if is_optional {
                        // For optional relations: Option<Option<ModelWithRelations>>
                        quote! { #name: Option<Option<#target::ModelWithRelations>> }
                    } else {
                        // For required relations: Option<ModelWithRelations>
                        quote! { #name: Option<#target::ModelWithRelations> }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation field names for initialization
    let relation_init_names = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate default values for relation fields
    let relation_defaults = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            quote! { #name: None }
        })
        .collect::<Vec<_>>();

    // Generate Filter and RelationFilter types
    let filter_types = quote! {
        pub struct Filter {
            pub field: String,
            pub value: String,
        }

        pub struct RelationFilter {
            pub relation: &'static str,
            pub filters: Vec<Filter>,
        }
    };

    // Generate ModelWithRelations struct and constructor
    let model_with_relations_impl = quote! {
        #filter_types

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct ModelWithRelations {
            #(#model_with_relations_fields,)*
            #(#relation_fields,)*
        }

        impl ModelWithRelations {
            pub fn new(
                #(#field_params,)*
                #(#relation_field_names,)*
            ) -> Self {
                Self {
                    #(#field_names,)*
                    #(#relation_init_names,)*
                }
            }

            pub fn from_model(model: Model) -> Self {
                Self {
                    #(#field_names: model.#field_names,)*
                    #(#relation_defaults,)*
                }
            }
        }

        impl std::default::Default for ModelWithRelations {
            fn default() -> Self {
                Self {
                    #(#field_names: Default::default(),)*
                    #(#relation_defaults,)*
                }
            }
        }

        impl caustics::FromModel<Model> for ModelWithRelations {
            fn from_model(model: Model) -> Self {
                Self::from_model(model)
            }
        }
    };

    // Generate relation connection match arms for SetParam
    let relation_connect_match_arms = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some()
        })
        .map(|relation| {
            let relation_name = format_ident!("Connect{}", relation.name.to_pascal_case());
            let foreign_key_field = format_ident!("{}", relation.foreign_key_field.as_ref().unwrap());
            let target_module = &relation.target;
            let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
            
            // Check if this is an optional relation
            let is_optional = if let Some(field) = fields.iter().find(|f| {
                f.ident.as_ref().unwrap().to_string() == *fk_field_name
            }) {
                is_option(&field.ty)
            } else {
                false
            };
            
            if is_optional {
                // For optional relations, handle Option<UniqueWhereParam>
                quote! {
                    SetParam::#relation_name(where_param_opt) => {
                        match where_param_opt {
                            Some(where_param) => {
                                // Convert UniqueWhereParam to foreign key value
                                let fk_value = match where_param {
                                    #target_module::UniqueWhereParam::IdEquals(id) => *id,
                                    _ => panic!("Only IdEquals is supported for foreign key relations"),
                                };
                                model.#foreign_key_field = sea_orm::ActiveValue::Set(Some(fk_value));
                            }
                            None => {
                                model.#foreign_key_field = sea_orm::ActiveValue::Set(None);
                            }
                        }
                    }
                }
            } else {
                // For required relations, handle UniqueWhereParam directly
                quote! {
                    SetParam::#relation_name(where_param) => {
                        // Convert UniqueWhereParam to foreign key value
                        let fk_value = match where_param {
                            #target_module::UniqueWhereParam::IdEquals(id) => *id,
                            _ => panic!("Only IdEquals is supported for foreign key relations"),
                        };
                        model.#foreign_key_field = sea_orm::ActiveValue::Set(fk_value);
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation disconnect match arms for SetParam
    let relation_disconnect_match_arms = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo) && 
            relation.foreign_key_field.is_some() && {
                // Only optional relations can be disconnected (set to None)
                let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                if let Some(field) = fields.iter().find(|f| {
                    f.ident.as_ref().unwrap().to_string() == *fk_field_name
                }) {
                    is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let relation_name = format_ident!("Disconnect{}", relation.name.to_pascal_case());
            let foreign_key_field = format_ident!("{}", relation.foreign_key_field.as_ref().unwrap());
            quote! {
                SetParam::#relation_name => {
                    model.#foreign_key_field = sea_orm::ActiveValue::Set(None);
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate match arms for SetParam (excluding primary keys)
    let match_arms = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            quote! {
                SetParam::#pascal_name(value) => {
                    model.#name = value.clone();
                }
            }
        })
        .collect::<Vec<_>>();

    // Combine all match arms
    let all_match_arms = quote! {
        #(#match_arms,)*
        #(#relation_connect_match_arms,)*
        #(#relation_disconnect_match_arms,)*
    };

    let expanded = {
        let required_struct_fields = required_struct_fields.clone();
        let required_fn_args = required_fn_args.clone();
        let required_inits = required_inits.clone();
        let required_assigns = required_assigns.clone();
        let all_set_param_variants = all_set_param_variants.clone();
        let all_match_arms = all_match_arms.clone();
        quote! {
            use sea_orm::{
                DatabaseConnection,
                Condition,
                EntityTrait,
                ActiveValue,
                QuerySelect,
                QueryOrder,
                QueryTrait,
                Select,
                ColumnTrait,
                IntoSimpleExpr,
                IntoActiveModel,
                ConnectionTrait,
            };
            use std::marker::PhantomData;
            use std::default::Default;
            use caustics::{SortOrder, MergeInto};

            pub struct EntityClient<'a, C: ConnectionTrait> {
                conn: &'a C
            }

            pub enum SetParam {
                #all_set_param_variants
            }

            pub enum WhereParam {
                #(#where_field_variants,)*
            }

            pub enum OrderByParam {
                #(#order_by_field_variants,)*
            }

            #[derive(Debug, Clone)]
            pub enum UniqueWhereParam {
                #(#unique_where_variants,)*
            }


            impl MergeInto<ActiveModel> for SetParam {
                fn merge_into(&self, model: &mut ActiveModel) {
                    match self {
                        #all_match_arms
                    }
                }
            }

            impl From<WhereParam> for Condition {
                fn from(param: WhereParam) -> Self {
                    match param {
                        #(#where_match_arms,)*
                    }
                }
            }

            impl From<UniqueWhereParam> for Condition {
                fn from(param: UniqueWhereParam) -> Self {
                    match param {
                        #(#unique_where_match_arms,)*
                    }
                }
            }

            impl From<OrderByParam> for (<Entity as EntityTrait>::Column, sea_orm::Order) {
                fn from(param: OrderByParam) -> Self {
                    match param {
                        #(#order_by_match_arms,)*
                    }
                }
            }

            pub struct Create {
                #(#required_struct_fields,)*
                #(#foreign_key_relation_fields,)*
                pub _params: Vec<SetParam>,
            }

            impl Create {
                pub fn new(#(#required_fn_args,)* #(#foreign_key_relation_args,)* _params: Vec<SetParam>) -> Self {
                    Self {
                        #(#required_inits,)*
                        #(#foreign_key_relation_inits,)*
                        _params,
                    }
                }

                fn into_active_model(self) -> ActiveModel {
                    let mut model = ActiveModel::new();
                    #(#required_assigns)*
                    #(#foreign_key_assigns)*
                    for opt in self._params {
                        opt.merge_into(&mut model);
                    }
                    model
                }
            }

            #model_with_relations_impl

            impl<'a, C: ConnectionTrait> EntityClient<'a, C> {
                pub fn new(conn: &'a C) -> Self {
                    Self { conn }
                }

                pub fn find_unique(&self, condition: UniqueWhereParam) -> caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
                    caustics::UniqueQueryBuilder {
                        query: <Entity as EntityTrait>::find().filter::<Condition>(condition.into()),
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn find_first(&self, conditions: Vec<WhereParam>) -> caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
                    let mut query = <Entity as EntityTrait>::find();
                    for cond in conditions {
                        query = query.filter::<Condition>(cond.into());
                    }
                    caustics::FirstQueryBuilder {
                        query,
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn find_many(&self, conditions: Vec<WhereParam>) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
                    let mut query = <Entity as EntityTrait>::find();
                    for cond in conditions {
                        query = query.filter::<Condition>(cond.into());
                    }
                    caustics::ManyQueryBuilder {
                        query,
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn create(&self, #(#required_fn_args,)* #(#foreign_key_relation_args,)* _params: Vec<SetParam>) -> caustics::CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations> {
                    let create = Create::new(#(#required_inits,)* #(#foreign_key_relation_inits,)* _params);
                    caustics::CreateQueryBuilder {
                        model: create.into_active_model(),
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn update(&self, condition: UniqueWhereParam, changes: Vec<SetParam>) -> caustics::UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam> {
                    caustics::UpdateQueryBuilder {
                        condition: condition.into(),
                        changes,
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn delete(&self, condition: UniqueWhereParam) -> caustics::DeleteQueryBuilder<'a, C, Entity> {
                    caustics::DeleteQueryBuilder {
                        condition: condition.into(),
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }

                pub fn upsert(&self, condition: UniqueWhereParam, create: Create, update: Vec<SetParam>) -> caustics::UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam> {
                    caustics::UpsertQueryBuilder {
                        condition: condition.into(),
                        create: create.into_active_model(),
                        update,
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    }
                }
            }

            #(#field_ops)*

            // Include the generated relation submodules
            #relation_submodules
        }
    };

    // Debug print the generated code
    let module_path = model_ast.ident.span().source_text().unwrap_or_default();
    eprintln!("Generated code for {}:\n{}", module_path, expanded);

    TokenStream::from(expanded)
}

fn extract_relations(relation_ast: &DeriveInput) -> Vec<Relation> {
    let mut relations = Vec::new();

    if let syn::Data::Enum(data_enum) = &relation_ast.data {
        for variant in &data_enum.variants {
            let mut foreign_key_field = None;
            let mut relation_name = None;
            let mut relation_target = None;
            let mut relation_kind = None;
            
            for attr in &variant.attrs {
                if let syn::Meta::List(meta) = &attr.meta {
                    if meta.path.is_ident("sea_orm") {
                        if let Ok(nested) = meta.parse_args_with(
                            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                        ) {
                            for meta in nested {
                                if let syn::Meta::NameValue(nv) = &meta {
                                    if nv.path.is_ident("has_many") || nv.path.is_ident("belongs_to") {
                                        if let syn::Expr::Lit(syn::ExprLit {
                                            lit: syn::Lit::Str(lit),
                                            ..
                                        }) = &nv.value
                                        {
                                            // Parse the target as a path
                                            let target_str = lit.value();
                                            let target_path = syn::parse_str::<syn::Path>(&target_str)
                                                .expect("Failed to parse relation target as path");

                                            // Create a new clean path without the "Entity" suffix
                                            let mut new_path = syn::Path {
                                                leading_colon: target_path.leading_colon,
                                                segments: syn::punctuated::Punctuated::new(),
                                            };
                                            
                                            // Copy all segments except the last one if it's "Entity"
                                            for (i, segment) in target_path.segments.iter().enumerate() {
                                                if i == target_path.segments.len() - 1 && segment.ident == "Entity" {
                                                    // Skip the "Entity" segment
                                                    continue;
                                                }
                                                new_path.segments.push(segment.clone());
                                            }

                                            relation_name = Some(variant.ident.to_string());
                                            relation_target = Some(new_path);
                                            relation_kind = Some(if nv.path.is_ident("has_many") {
                                                RelationKind::HasMany
                                            } else {
                                                RelationKind::BelongsTo
                                            });
                                        }
                                    } else if nv.path.is_ident("from") {
                                        if let syn::Expr::Lit(syn::ExprLit {
                                            lit: syn::Lit::Str(lit),
                                            ..
                                        }) = &nv.value
                                        {
                                            // Extract foreign key field name from "Column::FieldName"
                                            let column_str = lit.value();
                                            if let Some(field_name) = column_str.split("::").nth(1) {
                                                // Convert PascalCase to snake_case for field name
                                                let snake_case_name = field_name.to_string().to_snake_case();
                                                foreign_key_field = Some(snake_case_name);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Only add the relation if we have all the required information
            if let (Some(name), Some(target), Some(kind)) = (relation_name, relation_target, relation_kind) {
                relations.push(Relation {
                    name,
                    target,
                    kind,
                    foreign_key_field,
                });
            }
        }
    }

    relations
}

fn generate_relation_submodules(relations: &[Relation], fields: &[&syn::Field]) -> TokenStream {
    let mut submodules = Vec::new();

    for relation in relations {
        let relation_name = &relation.name;
        let relation_name_ident = format_ident!("{}", relation_name);
        let relation_name_lower = relation_name.to_lowercase();
        let relation_name_lower_ident = format_ident!("{}", relation_name_lower);
        let relation_name_str = relation_name;
        let target = &relation.target;

        let submodule = if matches!(relation.kind, RelationKind::BelongsTo) && relation.foreign_key_field.is_some() {
            // Check if this is an optional relation
            let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
            let is_optional = if let Some(field) = fields.iter().find(|f| {
                f.ident.as_ref().unwrap().to_string() == *fk_field_name
            }) {
                is_option(&field.ty)
            } else {
                false
            };
            
            if is_optional {
                // For optional relations, include connect and disconnect functionality
                let connect_variant = format_ident!("Connect{}", relation.name.to_pascal_case());
                let disconnect_variant = format_ident!("Disconnect{}", relation.name.to_pascal_case());
                
                quote! {
                    pub mod #relation_name_lower_ident {
                        pub fn fetch() -> super::RelationFilter {
                            super::RelationFilter {
                                relation: #relation_name_str,
                                filters: vec![],
                            }
                        }
                        
                        pub fn connect(where_param: super::#target::UniqueWhereParam) -> super::SetParam {
                            super::SetParam::#connect_variant(Some(where_param))
                        }
                        
                        pub fn disconnect() -> super::SetParam {
                            super::SetParam::#disconnect_variant
                        }
                    }
                }
            } else {
                // For required relations, only include fetch functionality (no connect/disconnect)
                quote! {
                    pub mod #relation_name_lower_ident {
                        pub fn fetch() -> super::RelationFilter {
                            super::RelationFilter {
                                relation: #relation_name_str,
                                filters: vec![],
                            }
                        }
                    }
                }
            }
        } else {
            // For relations without foreign keys (has_many), generate fetch with filters
            quote! {
                pub mod #relation_name_lower_ident {
                    pub fn fetch(filters: Vec<super::Filter>) -> super::RelationFilter {
                        super::RelationFilter {
                            relation: #relation_name_str,
                            filters,
                        }
                    }
                }
            }
        };

        submodules.push(submodule);
    }

    quote! {
        #(#submodules)*
    }
}
