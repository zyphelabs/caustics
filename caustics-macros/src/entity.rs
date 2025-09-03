use crate::common::is_option;
use crate::where_param::generate_where_param_logic;
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
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
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
    pub foreign_key_type: Option<syn::Type>,
    pub target_unique_param: Option<syn::Path>,
    pub is_nullable: bool,
    pub foreign_key_column: Option<String>,
    pub primary_key_field: Option<String>,
    pub current_table_name: Option<String>,
    pub target_table_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationKind {
    HasMany,
    BelongsTo,
}

pub fn generate_entity(
    model_ast: DeriveInput,
    relation_ast: DeriveInput,
    namespace: String,
    full_mod_path: &syn::Path,
) -> TokenStream {
    // Extract fields
    let fields = match &model_ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named.named.iter().collect::<Vec<_>>(),
            _ => {
                return quote! { compile_error!("#[caustics] requires a named-field struct for the Model"); };
            }
        },
        _ => {
            return quote! { compile_error!("#[caustics] must be applied to a struct"); };
        }
    };

    // Extract current entity's table name
    let current_table_name = extract_table_name(&model_ast);

    // Extract relations from relation_ast
    let relations = extract_relations(&relation_ast, &fields, &current_table_name);

    // Generate per-relation fetcher arms
    let mut relation_names = Vec::new();
    let mut relation_fetcher_bodies = Vec::new();
    for rel in &relations {
        let rel_name_snake = rel.name.to_snake_case();
        relation_names.push(quote! { #rel_name_snake });
        let target = &rel.target;
        let foreign_key_column = rel.foreign_key_column.as_ref().map_or("Id", |v| v);
        let foreign_key_column_ident = format_ident!("{}", foreign_key_column);
        let relation_name_str = rel.name.to_snake_case();

        let fetcher_body = if matches!(rel.kind, RelationKind::HasMany) {
            quote! {
            let mut query = #target::Entity::find()
                .filter(#target::Column::#foreign_key_column_ident.eq(foreign_key_value.unwrap_or_default()));

            // Apply cursor (id-based simple cursor)
            if let Some(cur) = filter.cursor_id {
                query = query.filter(#target::Column::Id.gt(cur));
            }

            // Apply order_by (support id only for now)
            for (field, dir) in &filter.order_by {
                if field == "id" {
                    let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                    query = query.order_by(#target::Column::Id, ord);
                }
            }

            if let Some(offset) = filter.skip { if offset > 0 { query = query.offset(offset as u64); } }
            if let Some(limit) = filter.take { if limit >= 0 { query = query.limit(limit as u64); } }

            let vec_with_rel = query.all(conn).await?
                        .into_iter()
                .map(|model| #target::ModelWithRelations::from_model(model))
                .collect::<Vec<_>>();

            Ok(Box::new(Some(vec_with_rel)) as Box<dyn std::any::Any + Send>)
                }
        } else {
            // belongs_to relation - query the TARGET entity by its primary key, using the current entity's foreign key value
            let is_nullable_fk = rel.is_nullable;
            let target_entity = &rel.target;
            let target_entity_type = quote! { #target_entity::Entity };
            let target_model_with_rel = quote! { #target_entity::ModelWithRelations };
            let target_unique_param = quote! { #target_entity::UniqueWhereParam };

            // Get the primary key field name from the relation definition or default to 'id'
            let primary_key_field_name = if let Some(pk) = &rel.primary_key_field {
                pk.as_str()
            } else {
                "id"
            };
            let primary_key_pascal = primary_key_field_name
                .chars()
                .next()
                .unwrap()
                .to_uppercase()
                .collect::<String>()
                + &primary_key_field_name[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);

            if is_nullable_fk {
                quote! {
                    if let Some(fk_value) = foreign_key_value {
                            let condition = #target_unique_param::#primary_key_variant(fk_value);
                            let opt_model = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into()).one(conn).await?;
                            let with_rel = opt_model.map(#target_model_with_rel::from_model);
                            let result: Option<Option<#target_model_with_rel>> = Some(with_rel);
                            return Ok(Box::new(result) as Box<dyn std::any::Any + Send>);
                        } else {
                            return Ok(Box::new(None::<Option<#target_model_with_rel>>) as Box<dyn std::any::Any + Send>);
                    }
                }
            } else {
                quote! {
                if let Some(fk_value) = foreign_key_value {
                        let condition = #target_unique_param::#primary_key_variant(fk_value);
                        let opt_model = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into()).one(conn).await?;
                        let with_rel = opt_model.map(#target_model_with_rel::from_model);
                        return Ok(Box::new(with_rel) as Box<dyn std::any::Any + Send>);
                } else {
                    Ok(Box::new(()) as Box<dyn std::any::Any + Send>)
                    }
                }
            }
        };
        relation_fetcher_bodies.push(fetcher_body);
    }

    // Compute at codegen time if this entity is the target of a has_many relation
    let is_has_many_target = relations
        .iter()
        .any(|rel| matches!(rel.kind, RelationKind::HasMany));

    // Compute if this entity has nullable foreign keys (for belongs_to relations)
    let has_nullable_foreign_keys = relations.iter().any(|rel| {
        matches!(rel.kind, RelationKind::BelongsTo)
            && rel.foreign_key_column.is_some()
            && rel.is_nullable
    });

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

    // Determine current entity primary key field ident (default to `id` if not annotated)
    let current_primary_key_ident = if let Some(pk_field) = primary_key_fields.first() {
        pk_field.ident.as_ref().unwrap().clone()
    } else {
        format_ident!("id")
    };

    // Filter out unique fields (including primary keys)
    let unique_fields: Vec<&syn::Field> = fields
        .iter()
        .cloned()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    (meta.path.is_ident("sea_orm")
                        && (meta.tokens.to_string().contains("primary_key")
                            || meta.tokens.to_string().contains("unique")))
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Check if the foreign key field is not nullable (not Option<T>)
                    // Only required relations should be in the Create struct
                    let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Check if the foreign key field is not nullable (not Option<T>)
                    // Only required relations should be function arguments
                    let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Check if the foreign key field is not nullable (not Option<T>)
                    // Only required relations should be initializers
                    let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
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

    // Generate unique field names as string literals for match arms
    let unique_field_names: Vec<_> = unique_fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            syn::LitStr::new(&field_name, field.ident.as_ref().unwrap().span())
        })
        .collect();

    // Generate unique field identifiers for column access (PascalCase for SeaORM)
    let unique_field_idents: Vec<_> = unique_fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            // Convert to PascalCase for SeaORM Column enum
            let pascal_case = field_name
                .chars()
                .next()
                .unwrap()
                .to_uppercase()
                .collect::<String>()
                + &field_name[1..];
            syn::Ident::new(&pascal_case, field.ident.as_ref().unwrap().span())
        })
        .collect();

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

            // Get the primary key field name from the relation definition or default to 'id'
            let primary_key_field_name = if let Some(pk) = &relation.primary_key_field {
                pk.as_str()
            } else {
                "id"
            };
            let primary_key_pascal = primary_key_field_name.chars().next().unwrap().to_uppercase().collect::<String>()
                + &primary_key_field_name[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);
            let primary_key_field_ident = format_ident!("{}", primary_key_field_name);

            quote! {
                // Handle foreign key value from UniqueWhereParam
                match self.#relation_name {
                    #target_module::UniqueWhereParam::#primary_key_variant(id) => {
                        model.#fk_field_ident = sea_orm::ActiveValue::Set(id.clone());
                    }
                    other => {
                        // For complex foreign key resolution, we need to add to deferred lookups
                        // This handles cases like user::email::equals(author.email)
                        deferred_lookups.push(caustics::DeferredLookup::new(
                            Box::new(other.clone()),
                            |model, value| {
                                let Some(model) = model.downcast_mut::<ActiveModel>() else {
                                    panic!("SetParam relation assign: ActiveModel type mismatch");
                                };
                                model.#fk_field_ident = sea_orm::ActiveValue::Set(value);
                            },
                            |conn: & sea_orm::DatabaseConnection, param| {
                                let Some(param) = param.downcast_ref::<#target_module::UniqueWhereParam>() else {
                                    panic!("Deferred FK: UniqueWhereParam type mismatch");
                                };
                                let param = param.clone();
                                Box::pin(async move {
                                    let condition: sea_query::Condition = param.clone().into();
                                    let result = #target_module::Entity::find()
                                        .filter::<sea_query::Condition>(condition)
                                        .one(conn)
                                        .await?;
                                    result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                        sea_orm::DbErr::Custom(format!(
                                            "No {} found for condition: {:?}",
                                            stringify!(#target_module),
                                            param
                                        ))
                                    })
                                })
                            },
                            |txn: & sea_orm::DatabaseTransaction, param| {
                                let Some(param) = param.downcast_ref::<#target_module::UniqueWhereParam>() else {
                                    panic!("Deferred FK: UniqueWhereParam type mismatch");
                                };
                                let param = param.clone();
                                Box::pin(async move {
                                    let condition: sea_query::Condition = param.clone().into();
                                    let result = #target_module::Entity::find()
                                        .filter::<sea_query::Condition>(condition)
                                        .one(txn)
                                        .await?;
                                    result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                        sea_orm::DbErr::Custom(format!(
                                            "No {} found for condition: {:?}",
                                            stringify!(#target_module),
                                            param
                                        ))
                                    })
                                })
                            },
                        ));
                    }
                }
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

    // Generate atomic operation variants for SetParam enum (for numeric fields only)
    let atomic_variants: Vec<_> = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .filter_map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let ty = &field.ty;
            // Check if this is a numeric field
            let field_type = crate::where_param::detect_field_type(ty);
            let is_numeric = matches!(
                field_type,
                crate::where_param::FieldType::Integer
                    | crate::where_param::FieldType::OptionInteger
                    | crate::where_param::FieldType::Float
                    | crate::where_param::FieldType::OptionFloat
            );
            if is_numeric {
                let inner_ty = crate::common::extract_inner_type_from_option(ty);
                let increment_name = format_ident!("{}Increment", pascal_name);
                let decrement_name = format_ident!("{}Decrement", pascal_name);
                let multiply_name = format_ident!("{}Multiply", pascal_name);
                let divide_name = format_ident!("{}Divide", pascal_name);

                Some(vec![
                    quote! { #increment_name(#inner_ty) },
                    quote! { #decrement_name(#inner_ty) },
                    quote! { #multiply_name(#inner_ty) },
                    quote! { #divide_name(#inner_ty) },
                ])
            } else {
                None
            }
        })
        .flatten()
        .collect();

    // Generate relation connection variants for SetParam enum
    let relation_connect_variants = relations
        .iter()
        .filter_map(|relation| {
            let relation_name = format_ident!("Connect{}", relation.name.to_pascal_case());
            let target_module = &relation.target;
            match relation.kind {
                RelationKind::BelongsTo => {
                    // Always take UniqueWhereParam, not Option<...>
                    Some(quote! {
                        #relation_name(#target_module::UniqueWhereParam)
                    })
                }
                RelationKind::HasMany => Some(quote! {
                    #relation_name(#target_module::UniqueWhereParam)
                }),
            }
        })
        .collect::<Vec<_>>();

    // Generate relation disconnect variants for SetParam enum
    let relation_disconnect_variants = relations
        .iter()
        .filter(|relation| {
            // Only include belongs_to relationships (where this entity has the foreign key)
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Only optional relations can be disconnected (set to None)
                    let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
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

    // Generate has_many set operation variants for SetParam enum
    let has_many_set_variants = relations
        .iter()
        .filter_map(|relation| {
            match relation.kind {
                RelationKind::HasMany => {
                    let relation_name = format_ident!("Set{}", relation.name.to_pascal_case());
                    let target_module = &relation.target;
                    Some((relation.name.clone(), relation_name, target_module.clone()))
                }
                _ => None
            }
        })
        .collect::<Vec<_>>();
    
    let has_many_set_variant_tokens = has_many_set_variants
        .iter()
        .map(|(_, relation_name, target_module)| {
            quote! {
                #relation_name(Vec<#target_module::UniqueWhereParam>)
            }
        })
        .collect::<Vec<_>>();

    // Combine all SetParam variants as a flat Vec
    let all_set_param_variants: Vec<_> = field_variants
        .into_iter()
        .chain(atomic_variants.into_iter())
        .chain(relation_connect_variants.into_iter())
        .chain(relation_disconnect_variants.into_iter())
        .chain(has_many_set_variant_tokens.into_iter())
        .collect();

    // Generate field variants and field operator modules for WhereParam enum (all fields, with string ops for string fields)
    let (where_field_variants, where_match_arms, field_ops) =
        generate_where_param_logic(&fields, &unique_fields, full_mod_path, &relations);

    // Generate match arms for UniqueWhereParam
    let unique_where_match_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let equals_variant = format_ident!("{}Equals", pascal_name);
            quote! {
                UniqueWhereParam::#equals_variant(value) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(value))
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate match arms to convert UniqueWhereParam into a cursor (expr, value)
    // Each arm evaluates to a new builder (Self)
    let unique_cursor_match_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let equals_variant = format_ident!("{}Equals", pascal_name);
            quote! {
                UniqueWhereParam::#equals_variant(value) => {
                    let expr = <Entity as EntityTrait>::Column::#pascal_name.into_simple_expr();
                    self.with_cursor(expr, sea_orm::Value::from(value))
                },
            }
        })
        .collect::<Vec<_>>();

    // Generate parallel lists of equals-variants and their columns for Into<(expr, value)>
    let unique_where_equals_variants = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            format_ident!("{}Equals", pascal_name)
        })
        .collect::<Vec<_>>();

    let unique_where_equals_columns = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            pascal_name
        })
        .collect::<Vec<_>>();

    // Generate field variants for OrderByParam enum (all fields)
    let order_by_field_variants = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            quote! {
                #pascal_name(caustics::SortOrder)
            }
        })
        .collect::<Vec<_>>();

    // Generate match arms for OrderByParam
    let order_by_match_arms = fields
        .iter()
        .map(|field| {
            let pascal_name = format_ident!(
                "{}",
                field.ident.as_ref().unwrap().to_string().to_pascal_case()
            );
            quote! {
                OrderByParam::#pascal_name(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::#pascal_name, sea_order)
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate variants for GroupByFieldParam (same set of fields as order-by, but without SortOrder)
    let group_by_field_variants = fields
        .iter()
        .map(|field| {
            let pascal_name = format_ident!(
                "{}",
                field.ident.as_ref().unwrap().to_string().to_pascal_case()
            );
            quote! { #pascal_name }
        })
        .collect::<Vec<_>>();

    // Generate snake_case function idents for per-entity select helpers
    let snake_field_fn_idents = fields
        .iter()
        .map(|field| {
            let snake = format_ident!(
                "{}",
                field.ident.as_ref().unwrap().to_string()
            );
            snake
        })
        .collect::<Vec<_>>();

    // Generate variants for GroupByOrderByParam (same as order_by_field_variants)
    let group_by_order_by_field_variants = order_by_field_variants.clone();

    // Generate match arms for GroupByOrderByParam -> (SimpleExpr, sea_orm::Order)
    let group_by_order_by_match_arms = fields
        .iter()
        .map(|field| {
            let pascal_name = format_ident!(
                "{}",
                field.ident.as_ref().unwrap().to_string().to_pascal_case()
            );
            quote! {
                GroupByOrderByParam::#pascal_name(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::#pascal_name.into_simple_expr(), sea_order)
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate aggregate select enums (typed per field)
    let sum_select_variants = fields.iter().map(|field| {
        let pascal_name = format_ident!(
            "{}",
            field.ident.as_ref().unwrap().to_string().to_pascal_case()
        );
        quote! { #pascal_name }
    }).collect::<Vec<_>>();
    let avg_select_variants = sum_select_variants.clone();
    let min_select_variants = sum_select_variants.clone();
    let max_select_variants = sum_select_variants.clone();

    // Generate typed WhereParam -> Filter conversion match arms (no string parsing)
    let filter_conversion_match_arms = fields
        .iter()
        .map(|field| {
            let name_ident = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name_ident.to_string().to_pascal_case());
            let field_name_lit = syn::LitStr::new(&name_ident.to_string(), name_ident.span());
            let is_opt = is_option(&field.ty);
            if is_opt {
                quote! {
                    WhereParam::#pascal_name(op) => {
                        let field = #field_name_lit.to_string();
                        let operation = match op {
                            caustics::FieldOp::Equals(v) => match v { Some(v) => caustics::FieldOp::Equals(v.to_string()), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::NotEquals(v) => match v { Some(v) => caustics::FieldOp::NotEquals(v.to_string()), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Gt(v) => match v { Some(v) => caustics::FieldOp::Gt(v.to_string()), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Lt(v) => match v { Some(v) => caustics::FieldOp::Lt(v.to_string()), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::Gte(v) => match v { Some(v) => caustics::FieldOp::Gte(v.to_string()), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Lte(v) => match v { Some(v) => caustics::FieldOp::Lte(v.to_string()), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::InVec(vs) => caustics::FieldOp::InVec(vs.into_iter().filter_map(|v| v.map(|x| x.to_string())).collect()),
                            caustics::FieldOp::NotInVec(vs) => caustics::FieldOp::NotInVec(vs.into_iter().filter_map(|v| v.map(|x| x.to_string())).collect()),
                            caustics::FieldOp::Contains(s) => caustics::FieldOp::Contains(s),
                            caustics::FieldOp::StartsWith(s) => caustics::FieldOp::StartsWith(s),
                            caustics::FieldOp::EndsWith(s) => caustics::FieldOp::EndsWith(s),
                            caustics::FieldOp::IsNull => caustics::FieldOp::IsNull,
                            caustics::FieldOp::IsNotNull => caustics::FieldOp::IsNotNull,
                            caustics::FieldOp::JsonPath(path) => caustics::FieldOp::JsonPath(path),
                            caustics::FieldOp::JsonStringContains(s) => caustics::FieldOp::JsonStringContains(s),
                            caustics::FieldOp::JsonStringStartsWith(s) => caustics::FieldOp::JsonStringStartsWith(s),
                            caustics::FieldOp::JsonStringEndsWith(s) => caustics::FieldOp::JsonStringEndsWith(s),
                            caustics::FieldOp::JsonArrayContains(v) => caustics::FieldOp::JsonArrayContains(v),
                            caustics::FieldOp::JsonArrayStartsWith(v) => caustics::FieldOp::JsonArrayStartsWith(v),
                            caustics::FieldOp::JsonArrayEndsWith(v) => caustics::FieldOp::JsonArrayEndsWith(v),
                            caustics::FieldOp::JsonObjectContains(s) => caustics::FieldOp::JsonObjectContains(s),
                            caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => unreachable!(),
                        };
                        caustics::Filter { field, operation }
                    }
                }
            } else {
                quote! {
                    WhereParam::#pascal_name(op) => {
                        let field = #field_name_lit.to_string();
                        let operation = match op {
                            caustics::FieldOp::Equals(v) => caustics::FieldOp::Equals(v.to_string()),
                            caustics::FieldOp::NotEquals(v) => caustics::FieldOp::NotEquals(v.to_string()),
                            caustics::FieldOp::Gt(v) => caustics::FieldOp::Gt(v.to_string()),
                            caustics::FieldOp::Lt(v) => caustics::FieldOp::Lt(v.to_string()),
                            caustics::FieldOp::Gte(v) => caustics::FieldOp::Gte(v.to_string()),
                            caustics::FieldOp::Lte(v) => caustics::FieldOp::Lte(v.to_string()),
                            caustics::FieldOp::InVec(vs) => caustics::FieldOp::InVec(vs.into_iter().map(|v| v.to_string()).collect()),
                            caustics::FieldOp::NotInVec(vs) => caustics::FieldOp::NotInVec(vs.into_iter().map(|v| v.to_string()).collect()),
                            caustics::FieldOp::Contains(s) => caustics::FieldOp::Contains(s),
                            caustics::FieldOp::StartsWith(s) => caustics::FieldOp::StartsWith(s),
                            caustics::FieldOp::EndsWith(s) => caustics::FieldOp::EndsWith(s),
                            caustics::FieldOp::IsNull => caustics::FieldOp::IsNull,
                            caustics::FieldOp::IsNotNull => caustics::FieldOp::IsNotNull,
                            caustics::FieldOp::JsonPath(path) => caustics::FieldOp::JsonPath(path),
                            caustics::FieldOp::JsonStringContains(s) => caustics::FieldOp::JsonStringContains(s),
                            caustics::FieldOp::JsonStringStartsWith(s) => caustics::FieldOp::JsonStringStartsWith(s),
                            caustics::FieldOp::JsonStringEndsWith(s) => caustics::FieldOp::JsonStringEndsWith(s),
                            caustics::FieldOp::JsonArrayContains(v) => caustics::FieldOp::JsonArrayContains(v),
                            caustics::FieldOp::JsonArrayStartsWith(v) => caustics::FieldOp::JsonArrayStartsWith(v),
                            caustics::FieldOp::JsonArrayEndsWith(v) => caustics::FieldOp::JsonArrayEndsWith(v),
                            caustics::FieldOp::JsonObjectContains(s) => caustics::FieldOp::JsonObjectContains(s),
                            caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => unreachable!(),
                        };
                        caustics::Filter { field, operation }
                    }
                }
            }
        })
        .collect::<Vec<_>>();


    // Generate UniqueWhereParam enum for unique fields
    let unique_where_variants = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = name.to_string().to_pascal_case();
            let equals_variant = format_ident!("{}Equals", pascal_name);
            let ty = &field.ty;
            quote! {
                #equals_variant(#ty)
            }
        })
        .collect::<Vec<_>>();

    // Generate all unique field variant id idents (e.g., IdEquals, EmailEquals)
    let unique_where_variant_idents: Vec<_> = unique_fields
        .iter()
        .map(|field| {
            let pascal_name = field.ident.as_ref().unwrap().to_string().to_pascal_case();
            format_ident!("{}Equals", pascal_name)
        })
        .collect();
    // Filter out the primary key variant (IdEquals)
    let other_unique_variants: Vec<_> = unique_where_variant_idents
        .iter()
        .filter(|ident| ident.to_string() != "IdEquals")
        .collect();

    // Generate UniqueWhereParam serialize implementation
    let unique_where_serialize_arms = unique_fields
        .iter()
        .map(|field| {
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
        })
        .collect::<Vec<_>>();

    // Generate field operator modules
    let field_ops = field_ops;

    // Generate relation submodules
    let relation_submodules = generate_relation_submodules(&relations, &fields);

    // Precompute nested-include pattern helpers
    let relation_names_snake_lits: Vec<_> = relations
        .iter()
        .map(|relation| {
            let name_str = relation.name.to_snake_case();
            syn::LitStr::new(&name_str, proc_macro2::Span::call_site())
        })
        .collect();
    let relation_nested_apply_blocks: Vec<_> = relations
        .iter()
        .map(|relation| {
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => {
                    quote! {
                        let vec_ref = fetched_result.downcast_mut::<Option<Vec<#target::ModelWithRelations>>>()
                            .expect("Type mismatch in nested has_many downcast");
                        if let Some(vec_inner) = vec_ref.as_mut() {
                            for elem in vec_inner.iter_mut() {
                                for nested in &filter.nested_includes {
                                    #target::ModelWithRelations::__caustics_apply_relation_filter(elem, conn, nested, registry).await?;
                                }
                            }
                        }
                    }
                }
                RelationKind::BelongsTo => {
                    // Determine optional vs required
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields
                            .iter()
                            .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                        {
                            is_option(&field.ty)
                        } else { false }
                    } else { false };
                    if is_optional {
                        quote! {
                            let mmref = fetched_result.downcast_mut::<Option<Option<#target::ModelWithRelations>>>()
                                .expect("Type mismatch in nested optional belongs_to downcast");
                            if let Some(inner) = mmref.as_mut() {
                                if let Some(model) = inner.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            }
                        }
                    } else {
                        quote! {
                            let mref = fetched_result.downcast_mut::<Option<#target::ModelWithRelations>>()
                                .expect("Type mismatch in nested belongs_to downcast");
                            if let Some(model) = mref.as_mut() {
                                for nested in &filter.nested_includes {
                                    #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                }
                            }
                        }
                    }
                }
            }
        })
        .collect();

    // Precompute take/skip apply blocks for has_many
    let relation_take_skip_blocks: Vec<_> = relations
        .iter()
        .map(|relation| {
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => {
                    quote! {
                        let vec_ref = fetched_result.downcast_mut::<Option<Vec<#target::ModelWithRelations>>>()
                            .expect("Type mismatch in has_many downcast (take/skip)");
                        if let Some(vec_inner) = vec_ref.as_mut() {
                            let len = vec_inner.len();
                            let start = filter.skip.unwrap_or(0).max(0) as usize;
                            let end = match filter.take { Some(t) if t >= 0 => (start + (t as usize)).min(len), _ => len };
                            if start >= len { vec_inner.clear(); } else if start > 0 || end < len {
                                let new_vec = vec_inner[start..end].to_vec();
                                *vec_inner = new_vec;
                            }
                        }
                    }
                }
                _ => quote! {},
            }
        })
        .collect();

    // Generate IncludeParam enum variants and match arms for include()
    let include_enum_variants = relations
        .iter()
        .map(|relation| {
            let variant = format_ident!("{}", relation.name.to_pascal_case());
            quote! { #variant(RelationFilter) }
        })
        .collect::<Vec<_>>();

    let include_match_arms = relations
        .iter()
        .map(|relation| {
            let variant = format_ident!("{}", relation.name.to_pascal_case());
            quote! { IncludeParam::#variant(filter) => { self = self.with(filter); } }
        })
        .collect::<Vec<_>>();

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
                RelationKind::HasMany => {
                    quote! { pub #name: Option<Vec<#target::ModelWithRelations>> }
                }
                RelationKind::BelongsTo => {
                    // Check if this is an optional relation by looking at the foreign key field
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields
                            .iter()
                            .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                        {
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
                        if let Some(field) = fields
                            .iter()
                            .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                        {
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
        pub type Filter = caustics::Filter;

        #[derive(Clone, Debug)]
        pub struct RelationFilter {
            pub relation: &'static str,
            pub filters: Vec<Filter>,
            pub nested_select_aliases: Option<Vec<String>>,
            pub nested_includes: Vec<caustics::RelationFilter>,
            pub take: Option<i64>,
            pub skip: Option<i64>,
            pub order_by: Vec<(String, caustics::SortOrder)>,
            pub cursor_id: Option<i32>,
            pub include_count: bool,
        }

        impl caustics::RelationFilterTrait for RelationFilter {
            fn relation_name(&self) -> &'static str {
                self.relation
            }

            fn filters(&self) -> &[caustics::Filter] {
                &self.filters
            }
        }

        impl From<RelationFilter> for caustics::RelationFilter {
            fn from(relation_filter: RelationFilter) -> Self {
                caustics::RelationFilter {
                    relation: relation_filter.relation,
                    filters: relation_filter.filters,
                    nested_select_aliases: relation_filter.nested_select_aliases,
                    nested_includes: relation_filter.nested_includes,
                    take: relation_filter.take,
                    skip: relation_filter.skip,
                    order_by: relation_filter.order_by,
                    cursor_id: relation_filter.cursor_id,
                    include_count: relation_filter.include_count,
                }
            }
        }
    };

    // Prepare Selected scalar field definitions (Option<InnerType>)
    let selected_scalar_fields = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let inner_ty = crate::common::extract_inner_type_from_option(&field.ty);
            quote! { pub #name: Option<#inner_ty> }
        })
        .collect::<Vec<_>>();

    // Generate per-field row extraction statements using snake_case aliases
    let selected_fill_stmts = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let inner_ty = crate::common::extract_inner_type_from_option(&field.ty);
            let alias = syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
            quote! { s.#name = row.try_get::<#inner_ty>("", #alias).ok(); }
        })
        .collect::<Vec<_>>();

    // Clear unselected scalar fields based on provided aliases (snake_case)
    let selected_clear_stmts = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let alias = syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
            quote! { if !allowed.contains(&#alias) { self.#name = None; } }
        })
        .collect::<Vec<_>>();

    // Match arms for get_i32 only for integer-like fields
    let get_i32_match_arms = fields
        .iter()
        .filter(|field| {
            matches!(
                crate::where_param::detect_field_type(&field.ty),
                crate::where_param::FieldType::Integer | crate::where_param::FieldType::OptionInteger
            )
        })
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let alias = syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
            quote! { #alias => self.#name }
        })
        .collect::<Vec<_>>();

    // Prepare alias/id pairs for Selected::column_for_alias
    let selected_all_field_names: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            syn::LitStr::new(&field_name, field.ident.as_ref().unwrap().span())
        })
        .collect();
    let selected_all_field_idents: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let pascal_case = field_name
                .split('_')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                    }
                })
                .collect::<String>();
            syn::Ident::new(&pascal_case, field.ident.as_ref().unwrap().span())
        })
        .collect();

    // Generate Counts struct fields for has_many relations
    let counts_struct_fields = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let name = format_ident!("{}", relation.name.to_snake_case());
                Some(quote! { pub #name: Option<i32> })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Precompute per-relation count arms used inside __caustics_apply_relation_filter
    let relation_count_match_arms = relations
        .iter()
        .map(|relation| {
            let relation_name_snake = relation.name.to_snake_case();
            let relation_name_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => {
                    let foreign_key_column = relation.foreign_key_column.as_ref().map_or("Id", |v| v);
                    let foreign_key_column_ident = format_ident!("{}", foreign_key_column);
                    let count_field_ident = format_ident!("{}", relation.name.to_snake_case());
                    quote! {
                        #relation_name_lit => {
                            if let Some(fkv) = foreign_key_value {
                                let total: i64 = #target::Entity::find()
                                    .filter(#target::Column::#foreign_key_column_ident.eq(fkv))
                                    .count(conn)
                                    .await?;
                                let mut c = self._count.take().unwrap_or_default();
                                c.#count_field_ident = Some(total as i32);
                                self._count = Some(c);
                            }
                        }
                    }
                }
                _ => quote! {},
            }
        })
        .collect::<Vec<_>>();

    // Generate ModelWithRelations struct and constructor
    let model_with_relations_impl = quote! {
        #filter_types

        #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
        pub struct Counts {
            #(#counts_struct_fields,)*
        }

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct ModelWithRelations {
            #(#model_with_relations_fields,)*
            #(#relation_fields,)*
            pub _count: Option<Counts>,
        }

        impl ModelWithRelations {
            pub fn new(
                #(#field_params,)*
                #(#relation_field_names,)*
            ) -> Self {
                Self {
                    #(#field_names,)*
                    #(#relation_init_names,)*
                    _count: None,
                }
            }

            pub fn from_model(model: Model) -> Self {
                Self {
                    #(#field_names: model.#field_names,)*
                    #(#relation_defaults,)*
                    _count: None,
                }
            }

            pub fn __caustics_apply_relation_filter<'a, C: sea_orm::ConnectionTrait>(
                &'a mut self,
                conn: &'a C,
                filter: &'a caustics::RelationFilter,
                registry: &'a (dyn caustics::EntityRegistry<C> + Sync),
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                Box::pin(async move {
                    let descriptor = <Self as caustics::HasRelationMetadata<Self>>::get_relation_descriptor(filter.relation)
                        .ok_or_else(|| caustics::CausticsError::RelationNotFound { relation: filter.relation.to_string() })?;
                    let foreign_key_value = (descriptor.get_foreign_key)(self);
                    // Always resolve fetcher for the current entity module
                    let fetcher_entity_name = {
                        let type_name = std::any::type_name::<Self>();
                        type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase()
                    };
                    let fetcher = registry.get_fetcher(&fetcher_entity_name)
                        .ok_or_else(|| caustics::CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;
                    let mut fetched_result = fetcher
                        .fetch_by_foreign_key(
                            conn,
                            foreign_key_value,
                            descriptor.foreign_key_column,
                            &fetcher_entity_name,
                            filter.relation,
                            filter,
                        )
                        .await?;

                    // In-memory order/take/skip removed; pushed down to SQL in fetcher

                    // relation counts not yet implemented

                    // Apply nested includes recursively, if any
                    if !filter.nested_includes.is_empty() {
                        match filter.relation {
                            #(
                                #relation_names_snake_lits => { #relation_nested_apply_blocks },
                            )*
                            _ => {}
                        }
                    }

                    (descriptor.set_field)(self, fetched_result);
                    Ok(())
                })
            }
        }

        impl<C: sea_orm::ConnectionTrait> caustics::ApplyNestedIncludes<C> for ModelWithRelations {
            fn apply_relation_filter<'a>(
                &'a mut self,
                conn: &'a C,
                filter: &'a caustics::RelationFilter,
                registry: &'a (dyn caustics::EntityRegistry<C> + Sync),
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                self.__caustics_apply_relation_filter(conn, filter, registry)
            }
        }

        impl std::default::Default for ModelWithRelations {
            fn default() -> Self {
                Self {
                    #(#field_names: Default::default(),)*
                    #(#relation_defaults,)*
                    _count: None,
                }
            }
        }

        impl caustics::FromModel<Model> for ModelWithRelations {
            fn from_model(model: Model) -> Self {
                Self::from_model(model)
            }
        }

        // Selected holder struct with Option<T> for all scalar fields and same relation fields
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
        pub struct Selected {
            #(#selected_scalar_fields,)*
            #(#relation_fields,)*
        }

        impl Selected { fn new() -> Self { Default::default() } }

        impl caustics::EntitySelection for Selected {
            fn fill_from_row(row: &sea_orm::QueryResult, _fields: &[&str]) -> Self {
                let mut s = Selected::new();
                #(#selected_fill_stmts)*
                s
            }

            fn clear_unselected(&mut self, allowed: &[&str]) {
                #(#selected_clear_stmts)*
            }

            fn set_relation(&mut self, relation_name: &str, value: Box<dyn std::any::Any + Send>) {
                match relation_name {
                    #( stringify!(#relation_init_names) => { let v = value.downcast().ok().expect("relation type"); self.#relation_init_names = *v; } ),*
                    _ => {}
                }
            }

            fn get_i32(&self, field_name: &str) -> Option<i32> {
                match field_name {
                    #(#get_i32_match_arms,)*
                    _ => None
                }
            }

            fn column_for_alias(alias: &str) -> Option<sea_query::SimpleExpr> {
                use sea_orm::IntoSimpleExpr;
                match alias {
                    #(
                        #selected_all_field_names => Some(<Entity as sea_orm::EntityTrait>::Column::#selected_all_field_idents.into_simple_expr()),
                    )*
                    _ => None,
                }
            }
        }

        impl Selected {
            pub fn __caustics_apply_relation_filter<'a, C: sea_orm::ConnectionTrait>(
                &'a mut self,
                conn: &'a C,
                filter: &'a caustics::RelationFilter,
                registry: &'a (dyn caustics::EntityRegistry<C> + Sync),
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                Box::pin(async move {
                    let descriptor = <Self as caustics::HasRelationMetadata<Selected>>::get_relation_descriptor(filter.relation)
                        .ok_or_else(|| caustics::CausticsError::RelationNotFound { relation: filter.relation.to_string() })?;
                    let foreign_key_value = (descriptor.get_foreign_key)(self);
                    let fetcher_entity_name = {
                        let type_name = std::any::type_name::<Self>();
                        type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase()
                    };
                    let fetcher = registry.get_fetcher(&fetcher_entity_name)
                        .ok_or_else(|| caustics::CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;
                    let mut fetched_result = fetcher
                        .fetch_by_foreign_key(
                            conn,
                            foreign_key_value,
                            descriptor.foreign_key_column,
                            &fetcher_entity_name,
                            filter.relation,
                            filter,
                        )
                        .await?;

                    // In-memory order/take/skip removed; pushed down to SQL in fetcher

                    // relation counts not yet implemented

                    if !filter.nested_includes.is_empty() {
                        match filter.relation {
                            #(
                                #relation_names_snake_lits => { #relation_nested_apply_blocks },
                            )*
                            _ => {}
                        }
                    }

                    (descriptor.set_field)(self, fetched_result);
                    Ok(())
                })
            }
        }

        impl<C: sea_orm::ConnectionTrait> caustics::ApplyNestedIncludes<C> for Selected {
            fn apply_relation_filter<'a>(
                &'a mut self,
                conn: &'a C,
                filter: &'a caustics::RelationFilter,
                registry: &'a (dyn caustics::EntityRegistry<C> + Sync),
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                self.__caustics_apply_relation_filter(conn, filter, registry)
            }
        }
    };

    // --- Begin relation metadata generation ---
    let relation_descriptors = relations.iter().map(|relation| {
        let rel_field = format_ident!("{}", relation.name.to_snake_case());
        let name_str = relation.name.to_snake_case();
        let name = syn::LitStr::new(&name_str, proc_macro2::Span::call_site());
        let target = &relation.target;
        let rel_type = match relation.kind {
            RelationKind::HasMany => quote! { Option<Vec<#target::ModelWithRelations>> },
            RelationKind::BelongsTo => {
                // Check if this is an optional relation by looking at the foreign key field
                let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
                        is_option(&field.ty)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if is_optional {
                    // For optional relations: Option<Option<ModelWithRelations>>
                    quote! { Option<Option<#target::ModelWithRelations>> }
                } else {
                    // For required relations: Option<ModelWithRelations>
                    quote! { Option<#target::ModelWithRelations> }
                }
            }
        };
        // Determine foreign key field and column based on relation type
        let (foreign_key_field, foreign_key_column, get_foreign_key_closure) = match relation.kind {
            RelationKind::HasMany => {
                let id_field = format_ident!("id");
                // For HasMany relations, the foreign key column is in the target entity
                // Use the extracted foreign_key_column if available, otherwise fallback to mapping
                let fk_column = if let Some(fk_col) = &relation.foreign_key_column {
                    // Convert PascalCase to snake_case to match database column names
                    // This is completely dynamic and works with any foreign key column name
                    fk_col.to_snake_case()
                } else {
                    // Fallback: use the relation name + "_id" pattern
                    // This is also dynamic and works with any relation name
                    format!("{}_id", relation.name.to_snake_case())
                };
                (
                    quote! { model.#id_field },
                    fk_column,
                    quote! { |model| Some(model.id) },
                )
            }
            RelationKind::BelongsTo => {
                // Use the foreign key field from the relation definition
                let foreign_key_field_name = relation
                    .foreign_key_field
                    .as_ref()
                    .expect("BelongsTo relation must have foreign_key_field defined");
                let foreign_key_field = format_ident!("{}", foreign_key_field_name);
                let is_optional = if let Some(field) = fields
                    .iter()
                    .find(|f| f.ident.as_ref().unwrap().to_string() == *foreign_key_field_name)
                {
                    is_option(&field.ty)
                } else {
                    false
                };
                let get_fk = if is_optional {
                    quote! { |model| model.#foreign_key_field }
                } else {
                    quote! { |model| Some(model.#foreign_key_field) }
                };
                (
                    quote! { model.#foreign_key_field },
                    foreign_key_field_name.to_string(),
                    get_fk,
                )
            }
        };
        // Use the lowercase module name as the registry key (e.g., "post")
        let target_entity_module_name_lower = relation
            .target
            .segments
            .last()
            .unwrap()
            .ident
            .to_string()
            .to_lowercase();
        let target_entity = syn::LitStr::new(
            &target_entity_module_name_lower,
            proc_macro2::Span::call_site(),
        );
        let foreign_key_column = syn::LitStr::new(&foreign_key_column, proc_macro2::Span::call_site());
        
        // Get additional metadata from relation
        let fallback_table_name = relation.name.to_snake_case();
        let target_table_name = relation
            .target_table_name
            .as_ref()
            .unwrap_or(&fallback_table_name);
        let unknown_table = "unknown".to_string();
        let current_table_name = relation
            .current_table_name
            .as_ref()
            .unwrap_or(&unknown_table);
        
        let target_table_name_lit = syn::LitStr::new(target_table_name, proc_macro2::Span::call_site());
        let current_table_name_lit = syn::LitStr::new(current_table_name, proc_macro2::Span::call_site());
        // Extract primary key column names dynamically (default to "id" if not annotated)
        let current_primary_key_column = if let Some(pk_field) = primary_key_fields.first() {
            pk_field.ident.as_ref().unwrap().to_string()
        } else {
            "id".to_string()
        };
        let current_primary_key_column_lit = syn::LitStr::new(&current_primary_key_column, proc_macro2::Span::call_site());
        
        // For target primary key, use the relation's primary_key_field or default to "id"
        let target_primary_key_column = if let Some(pk_field) = &relation.primary_key_field {
            pk_field.clone()
        } else {
            "id".to_string()
        };
        let target_primary_key_column_lit = syn::LitStr::new(&target_primary_key_column, proc_macro2::Span::call_site());
        let is_foreign_key_nullable_lit = syn::LitBool::new(relation.is_nullable, proc_macro2::Span::call_site());
        
        let fk_field_name_lit = match relation.kind {
            RelationKind::HasMany => syn::LitStr::new("id", proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitStr::new(relation.foreign_key_field.as_ref().unwrap(), proc_macro2::Span::call_site()),
        };
        let current_primary_key_field_name_lit = syn::LitStr::new(&current_primary_key_column, proc_macro2::Span::call_site());
        let is_has_many_lit = match relation.kind {
            RelationKind::HasMany => syn::LitBool::new(true, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitBool::new(false, proc_macro2::Span::call_site()),
        };
        quote! {
            caustics::RelationDescriptor::<ModelWithRelations> {
                name: #name,
                set_field: |model, value| {
                    let value = value.downcast::<#rel_type>().expect("Type mismatch in set_field");
                    model.#rel_field = *value;
                },
                get_foreign_key: #get_foreign_key_closure,
                target_entity: #target_entity,
                foreign_key_column: #foreign_key_column,
                foreign_key_field_name: #fk_field_name_lit,
                target_table_name: #target_table_name_lit,
                current_primary_key_column: #current_primary_key_column_lit,
                current_primary_key_field_name: #current_primary_key_field_name_lit,
                target_primary_key_column: #target_primary_key_column_lit,
                is_foreign_key_nullable: #is_foreign_key_nullable_lit,
                is_has_many: #is_has_many_lit,
            }
        }
    });

    // Also build relation descriptors for Selected (uses Option<T> scalars)
    let selected_relation_descriptors = relations.iter().map(|relation| {
        let rel_field = format_ident!("{}", relation.name.to_snake_case());
        let name_str = relation.name.to_snake_case();
        let name = syn::LitStr::new(&name_str, proc_macro2::Span::call_site());
        let target = &relation.target;
        let rel_type = match relation.kind {
            RelationKind::HasMany => quote! { Option<Vec<#target::ModelWithRelations>> },
            RelationKind::BelongsTo => {
                let is_optional = relation.is_nullable;
                if is_optional { quote! { Option<Option<#target::ModelWithRelations>> } } else { quote! { Option<#target::ModelWithRelations> } }
            }
        };
        let foreign_key_column = relation.foreign_key_column.as_ref().map(|s| s.clone()).unwrap_or_else(|| "id".to_string());
        let target_entity_module_name_lower = relation
            .target
            .segments
            .last()
            .unwrap()
            .ident
            .to_string()
            .to_lowercase();
        let target_entity = syn::LitStr::new(&target_entity_module_name_lower, proc_macro2::Span::call_site());
        let foreign_key_column = syn::LitStr::new(&foreign_key_column, proc_macro2::Span::call_site());
        let fk_field_name_lit = match relation.kind {
            RelationKind::HasMany => syn::LitStr::new("id", proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitStr::new(relation.foreign_key_field.as_ref().unwrap(), proc_macro2::Span::call_site()),
        };
        let target_table_default = relation.name.to_snake_case();
        let target_table_name_ref = relation
            .target_table_name
            .as_ref()
            .unwrap_or(&target_table_default);
        let target_table_name_lit = syn::LitStr::new(target_table_name_ref, proc_macro2::Span::call_site());
        let current_primary_key_field_name_lit = syn::LitStr::new("id", proc_macro2::Span::call_site());
        let current_primary_key_column_lit = syn::LitStr::new("id", proc_macro2::Span::call_site());
        let target_primary_key_column_lit = syn::LitStr::new(&relation
            .primary_key_field
            .as_ref()
            .map(|s| s.clone())
            .unwrap_or_else(|| "id".to_string()), proc_macro2::Span::call_site());
        let is_has_many_lit = match relation.kind {
            RelationKind::HasMany => syn::LitBool::new(true, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitBool::new(false, proc_macro2::Span::call_site()),
        };
        quote! {
            caustics::RelationDescriptor::<Selected> {
                name: #name,
                set_field: |model, value| {
                    let value = value.downcast::<#rel_type>().expect("Type mismatch in set_field");
                    model.#rel_field = *value;
                },
                get_foreign_key: |model: &Selected| {
                    // For has_many, use current id; for belongs_to, use FK field on Selected
                    let field_name = match #is_has_many_lit { true => "id", false => #fk_field_name_lit };
                    <Selected as caustics::EntitySelection>::get_i32(model, field_name)
                },
                target_entity: #target_entity,
                foreign_key_column: #foreign_key_column,
                foreign_key_field_name: #fk_field_name_lit,
                target_table_name: #target_table_name_lit,
                current_primary_key_column: #current_primary_key_column_lit,
                current_primary_key_field_name: #current_primary_key_field_name_lit,
                target_primary_key_column: #target_primary_key_column_lit,
                is_foreign_key_nullable: #is_has_many_lit,
                is_has_many: #is_has_many_lit,
            }
        }
    });

    let relation_metadata_impl = quote! {
        static RELATION_DESCRIPTORS: &[caustics::RelationDescriptor<ModelWithRelations>] = &[
            #(#relation_descriptors,)*
        ];
        impl caustics::HasRelationMetadata<ModelWithRelations> for ModelWithRelations {
            fn relation_descriptors() -> &'static [caustics::RelationDescriptor<ModelWithRelations>] {
                RELATION_DESCRIPTORS
            }
        }

        static SELECTED_RELATION_DESCRIPTORS: &[caustics::RelationDescriptor<Selected>] = &[
            #(#selected_relation_descriptors,)*
        ];
        impl caustics::HasRelationMetadata<Selected> for Selected {
            fn relation_descriptors() -> &'static [caustics::RelationDescriptor<Selected>] {
                SELECTED_RELATION_DESCRIPTORS
            }
        }
    };

    // Generate relation connection match arms for SetParam (for deferred lookups)
    let relation_connect_deferred_match_arms = relations
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
            let target_entity_name = relation.target.segments.last().unwrap().ident.to_string().to_lowercase();

            // Get the primary key field name from the relation definition or default to 'id'
            let primary_key_field_name = if let Some(pk) = &relation.primary_key_field {
                pk.as_str()
            } else {
                "id"
            };
            let primary_key_pascal = primary_key_field_name.chars().next().unwrap().to_uppercase().collect::<String>()
                + &primary_key_field_name[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);
            let primary_key_field_ident = format_ident!("{}", primary_key_field_name);

            // Check if this is an optional relation
            let is_optional = if let Some(field) = fields.iter().find(|f| {
                f.ident.as_ref().unwrap().to_string() == *fk_field_name
            }) {
                is_option(&field.ty)
            } else {
                false
            };

            if is_optional {
                quote! {
                    SetParam::#relation_name(where_param) => {
                        match where_param {
                            #target_module::UniqueWhereParam::#primary_key_variant(id) => {
                                model.#foreign_key_field = sea_orm::ActiveValue::Set(Some(id.clone()));
                            }
                            other => {
                                // Store deferred lookup instead of executing (optional FK -> wrap in Some)
                                deferred_lookups.push(caustics::DeferredLookup::new(
                                    Box::new(other.clone()),
                                    |model, value| {
                                        let model = model.downcast_mut::<ActiveModel>().unwrap();
                                        model.#foreign_key_field = sea_orm::ActiveValue::Set(Some(value));
                                    },
                                    |conn: & sea_orm::DatabaseConnection, param| {
                                        let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                        Box::pin(async move {
                                            let condition: sea_query::Condition = param.clone().into();
                                            let result = #target_module::Entity::find()
                                                .filter::<sea_query::Condition>(condition)
                                                .one(conn)
                                                .await?;
                                            result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                                sea_orm::DbErr::Custom(format!(
                                                    "No {} found for condition: {:?}",
                                                    stringify!(#target_module),
                                                    param
                                                ))
                                            })
                                        })
                                    },
                                    |txn: & sea_orm::DatabaseTransaction, param| {
                                        let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                        Box::pin(async move {
                                            let condition: sea_query::Condition = param.clone().into();
                                            let result = #target_module::Entity::find()
                                                .filter::<sea_query::Condition>(condition)
                                                .one(txn)
                                                .await?;
                                            result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                                sea_orm::DbErr::Custom(format!(
                                                    "No {} found for condition: {:?}",
                                                    stringify!(#target_module),
                                                    param
                                                ))
                                            })
                                        })
                                    },
                                ));
                            }
                        }
                    }
                }
            } else {
                quote! {
                    SetParam::#relation_name(where_param) => {
                        match where_param {
                            #target_module::UniqueWhereParam::#primary_key_variant(id) => {
                                model.#foreign_key_field = sea_orm::ActiveValue::Set(id.clone());
                            }
                            other => {
                                // Store deferred lookup instead of executing
                                                        deferred_lookups.push(caustics::DeferredLookup::new(
                            Box::new(other.clone()),
                            |model, value| {
                                let model = model.downcast_mut::<ActiveModel>().unwrap();
                                model.#foreign_key_field = sea_orm::ActiveValue::Set(value);
                            },
                                     |conn: & sea_orm::DatabaseConnection, param| {
                                        let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                        Box::pin(async move {
                                            let condition: sea_query::Condition = param.clone().into();
                                            let result = #target_module::Entity::find()
                                                .filter::<sea_query::Condition>(condition)
                                                .one(conn)
                                                .await?;
                                            result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                                sea_orm::DbErr::Custom(format!(
                                                    "No {} found for condition: {:?}",
                                                    stringify!(#target_module),
                                                    param
                                                ))
                                            })
                                        })
                                    },
                                     |txn: & sea_orm::DatabaseTransaction, param| {
                                         let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                         Box::pin(async move {
                                             let condition: sea_query::Condition = param.clone().into();
                                             let result = #target_module::Entity::find()
                                                 .filter::<sea_query::Condition>(condition)
                                                 .one(txn)
                                                .await?;
                                            result.map(|entity| entity.#primary_key_field_ident).ok_or_else(|| {
                                                sea_orm::DbErr::Custom(format!(
                                                    "No {} found for condition: {:?}",
                                                    stringify!(#target_module),
                                                    param
                                                ))
                                            })
                                        })
                                    },
                                ));
                            }
                        }
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Only optional relations can be disconnected (set to None)
                    let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
                        is_option(&field.ty)
                    } else {
                        false
                    }
                }
        })
        .map(|relation| {
            let relation_name = format_ident!("Disconnect{}", relation.name.to_pascal_case());
            let foreign_key_field =
                format_ident!("{}", relation.foreign_key_field.as_ref().unwrap());
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

    // Generate atomic operation match arms for SetParam (for numeric fields only)
    let atomic_match_arms: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .filter(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            !foreign_key_fields.contains(&field_name)
        })
        .filter_map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let ty = &field.ty;

            // Check if this is a numeric field
            let field_type = crate::where_param::detect_field_type(ty);
            let is_numeric = matches!(
                field_type,
                crate::where_param::FieldType::Integer
                    | crate::where_param::FieldType::OptionInteger
                    | crate::where_param::FieldType::Float
                    | crate::where_param::FieldType::OptionFloat
            );

            if is_numeric {
                let is_nullable = matches!(
                    field_type,
                    crate::where_param::FieldType::OptionInteger
                        | crate::where_param::FieldType::OptionFloat
                );

                // Create identifiers for atomic operation variants
                let increment_name = format_ident!("{}Increment", pascal_name);
                let decrement_name = format_ident!("{}Decrement", pascal_name);
                let multiply_name = format_ident!("{}Multiply", pascal_name);
                let divide_name = format_ident!("{}Divide", pascal_name);

                if is_nullable {
                    // For nullable fields, we need to handle the Option wrapper
                    // Try a very simple atomic operation
                    Some(vec![
                        quote! {
                            SetParam::#increment_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(Some(current_val)) => {
                                        Some(current_val + *value)
                                    },
                                    sea_orm::ActiveValue::Set(None) => Some(*value),
                                    sea_orm::ActiveValue::Unchanged(Some(current_val)) => {
                                        Some(current_val + *value)
                                    },
                                    sea_orm::ActiveValue::Unchanged(None) => Some(*value),
                                    _ => Some(*value),
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#decrement_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(Some(current_val)) => {
                                        Some(current_val - *value)
                                    },
                                    sea_orm::ActiveValue::Set(None) => Some(-*value),
                                    sea_orm::ActiveValue::Unchanged(Some(current_val)) => {
                                        Some(current_val - *value)
                                    },
                                    sea_orm::ActiveValue::Unchanged(None) => Some(-*value),
                                    _ => Some(-*value),
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#multiply_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(Some(current_val)) => {
                                        Some(current_val * *value)
                                    },
                                    sea_orm::ActiveValue::Unchanged(Some(current_val)) => {
                                        Some(current_val * *value)
                                    },
                                    _ => None,
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#divide_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(Some(current_val)) => {
                                        Some(current_val / *value)
                                    },
                                    sea_orm::ActiveValue::Unchanged(Some(current_val)) => {
                                        Some(current_val / *value)
                                    },
                                    _ => None,
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                    ])
                } else {
                    // For non-nullable fields
                    Some(vec![
                        quote! {
                            SetParam::#increment_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(val) => {
                                        // val is i32, value is &i32
                                        val + *value
                                    },
                                    sea_orm::ActiveValue::NotSet => *value,
                                    sea_orm::ActiveValue::Unchanged(val) => {
                                        val + *value
                                    },
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#decrement_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(val) => {
                                        val - *value
                                    },
                                    sea_orm::ActiveValue::NotSet => -*value,
                                    sea_orm::ActiveValue::Unchanged(val) => {
                                        val - *value
                                    },
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#multiply_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(val) => {
                                        val * *value
                                    },
                                    sea_orm::ActiveValue::NotSet => 0,
                                    sea_orm::ActiveValue::Unchanged(val) => {
                                        val * *value
                                    },
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                        quote! {
                            SetParam::#divide_name(value) => {
                                let current = model.#name.clone();
                                let new_value = match current {
                                    sea_orm::ActiveValue::Set(val) => {
                                        val / *value
                                    },
                                    sea_orm::ActiveValue::NotSet => 0,
                                    sea_orm::ActiveValue::Unchanged(val) => {
                                        val / *value
                                    },
                                };
                                model.#name = sea_orm::ActiveValue::Set(new_value);
                            }
                        },
                    ])
                }
            } else {
                None
            }
        })
        .flatten()
        .collect();

    // Generate SetParamInfo trait match arms
    let has_many_set_match_arms = has_many_set_variants
        .iter()
        .map(|(_, relation_name, _)| {
            quote! { SetParam::#relation_name(_) => true }
        })
        .collect::<Vec<_>>();
    
    let relation_name_match_arms = has_many_set_variants
        .iter()
        .map(|(relation_name, variant_name, _)| {
            let relation_name_lit = syn::LitStr::new(&relation_name.to_lowercase(), proc_macro2::Span::call_site());
            quote! { SetParam::#variant_name(_) => Some(#relation_name_lit) }
        })
        .collect::<Vec<_>>();
    
    let target_ids_match_arms = has_many_set_variants
        .iter()
        .map(|(_, variant_name, target_module)| {
            quote! {
                SetParam::#variant_name(unique_params) => {
                    // Extract IDs from Vec<#target_module::UniqueWhereParam>
                    // Parse each UniqueWhereParam to extract the ID
                    let mut target_ids = Vec::new();
                    for unique_param in unique_params {
                        // Convert UniqueWhereParam to string and extract ID
                        let param_str = format!("{:?}", unique_param);
                        if let Some(id_start) = param_str.find("Equals(") {
                            let after_equals = &param_str[id_start + 7..];
                            if let Some(id_end) = after_equals.find(')') {
                                let id_str = &after_equals[..id_end];
                                if let Ok(id) = id_str.parse::<i32>() {
                                    target_ids.push(sea_orm::Value::Int(Some(id)));
                                }
                            }
                        }
                    }
                    target_ids
                }
            }
        })
        .collect::<Vec<_>>();

    // Combine all match arms
    let all_match_arms = quote! {
        #(#match_arms,)*
        #(#atomic_match_arms,)*
        #(#relation_connect_deferred_match_arms,)*
        #(#relation_disconnect_match_arms,)*
    };

    let entity_name_lit = syn::LitStr::new(&model_ast.ident.to_string(), model_ast.ident.span());
    // Generate all field names as string literals for match arms
    let all_field_names: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            syn::LitStr::new(&field_name, field.ident.as_ref().unwrap().span())
        })
        .collect();
    // Generate all field identifiers for column access (PascalCase for SeaORM)
    let all_field_idents: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            // Convert snake_case to PascalCase
            let pascal_case = field_name
                .split('_')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                    }
                })
                .collect::<String>();
            syn::Ident::new(&pascal_case, field.ident.as_ref().unwrap().span())
        })
        .collect();
    // Generate the column_from_str function using the variables
    let column_from_str_fn = quote! {
        pub(crate) fn column_from_str(name: &str) -> Option<<Entity as sea_orm::EntityTrait>::Column> {
            match name {
                #(
                    #all_field_names => Some(<Entity as sea_orm::EntityTrait>::Column::#all_field_idents),
                )*
                _ => None,
            }
        }
    };

    let namespace_ident = format_ident!("{}", namespace);

    let expanded = quote! {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use caustics::{SortOrder, MergeInto, FieldOp};
        use caustics::FromModel;
        use sea_query::{Condition, Expr, SimpleExpr};
        use sea_orm::{ColumnTrait, IntoSimpleExpr, QueryFilter, QueryOrder, QuerySelect};
        use serde_json;
        use std::sync::Arc;
        use heck::ToSnakeCase;

        pub struct EntityClient<'a, C: sea_orm::ConnectionTrait> {
            conn: &'a C,
            database_backend: sea_orm::DatabaseBackend,
        }

        // Centralize registry selection to avoid scattered cfg(test) blocks
        #[allow(dead_code)]
        fn __caustics_fetch_registry<'a>() -> &'a super::CompositeEntityRegistry {
            #[cfg(test)]
            { super::get_registry() }
            #[cfg(not(test))]
            { crate::get_registry() }
        }

        #[derive(Debug)]
        pub enum SetParam {
            #(#all_set_param_variants,)*
        }
        #[derive(Debug, Clone)]
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

        #[derive(Debug, Clone)]
        pub enum GroupByFieldParam {
            #(#group_by_field_variants,)*
        }

        #[derive(Debug, Clone)]
        pub enum DistinctFieldParam {
            #(#group_by_field_variants,)*
        }

        // PCR-like scalar field enum alias
        #[derive(Debug, Clone)]
        pub enum ScalarField {
            #(#group_by_field_variants,)*
        }

        // Select parameters for scalar fields (PCR-like select)
        #[derive(Debug, Clone)]
        pub enum SelectParam {
            #(#group_by_field_variants,)*
        }

        // Per-entity snake_case select helpers, e.g. user::select::id()
        pub mod select {
            use super::SelectParam;
            #(pub fn #snake_field_fn_idents() -> SelectParam { SelectParam::#group_by_field_variants })*
        }

        // Map typed SelectParam to column alias strings (snake_case)
        pub fn select_params_to_aliases(params: Vec<SelectParam>) -> Vec<String> {
            let mut out: Vec<String> = Vec::with_capacity(params.len());
            for p in params {
                match p {
                    #( SelectParam::#group_by_field_variants => out.push(#all_field_names.to_string()), )*
                }
            }
            out
        }

        // Extension traits to apply select on query builders returning Selected builders
        pub trait ManySelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
        }

        impl<'a, C> ManySelectExt<'a, C> for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected> {
                use sea_orm::IntoSimpleExpr;
                let mut builder = caustics::SelectManyQueryBuilder {
                    query: self.query,
                    conn: self.conn,
                    selected_fields: Vec::new(),
                    requested_aliases: Vec::new(),
                    relations_to_fetch: self.relations_to_fetch,
                    registry: self.registry,
                    database_backend: self.database_backend,
                    reverse_order: self.reverse_order,
                    pending_order_bys: self.pending_order_bys,
                    cursor: self.cursor,
                    is_distinct: self.is_distinct,
                    distinct_on_fields: self.distinct_on_fields,
                    skip_is_negative: false,
                    _phantom: std::marker::PhantomData,
                };
                for s in selects {
                    match s {
                        #( SelectParam::#group_by_field_variants => {
                            let expr = <Entity as sea_orm::EntityTrait>::Column::#group_by_field_variants.into_simple_expr();
                            builder = builder.push_field(expr, #all_field_names);
                            builder.requested_aliases.push(#all_field_names.to_string());
                        } ),*
                    }
                }
                builder
            }
        }

        pub trait UniqueSelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>;
        }

        impl<'a, C> UniqueSelectExt<'a, C> for caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected> {
                use sea_orm::IntoSimpleExpr;
                let mut builder = caustics::SelectUniqueQueryBuilder {
                    query: self.query,
                    conn: self.conn,
                    selected_fields: Vec::new(),
                    requested_aliases: Vec::new(),
                    relations_to_fetch: self.relations_to_fetch,
                    registry: self.registry,
                    database_backend: self.conn.get_database_backend(),
                    _phantom: std::marker::PhantomData,
                };
                for s in selects {
                    match s {
                        #( SelectParam::#group_by_field_variants => {
                            let expr = <Entity as sea_orm::EntityTrait>::Column::#group_by_field_variants.into_simple_expr();
                            builder = builder.push_field(expr, #all_field_names);
                            builder.requested_aliases.push(#all_field_names.to_string());
                        } ),*
                    }
                }
                builder
            }
        }

        pub trait FirstSelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>;
        }

        impl<'a, C> FirstSelectExt<'a, C> for caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select(self, selects: Vec<SelectParam>) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected> {
                use sea_orm::IntoSimpleExpr;
                let mut builder = caustics::SelectFirstQueryBuilder {
                    query: self.query,
                    conn: self.conn,
                    selected_fields: Vec::new(),
                    requested_aliases: Vec::new(),
                    relations_to_fetch: self.relations_to_fetch,
                    registry: self.registry,
                    database_backend: self.database_backend,
                    _phantom: std::marker::PhantomData,
                };
                for s in selects {
                    match s {
                        #( SelectParam::#group_by_field_variants => {
                            let expr = <Entity as sea_orm::EntityTrait>::Column::#group_by_field_variants.into_simple_expr();
                            builder = builder.push_field(expr, #all_field_names);
                            builder.requested_aliases.push(#all_field_names.to_string());
                        } ),*
                    }
                }
                builder
            }
        }

        // Include parameters for relations (PCR-like include)
        #[derive(Debug, Clone)]
        pub enum IncludeParam {
            #(#include_enum_variants,)*
        }

        // Legacy include extension methods removed in favor of `.with(...)`

        // Include on select builders
        pub trait SelectManyIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
        }
        impl<'a, C> SelectManyIncludeExt<'a, C> for caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>
        where C: sea_orm::ConnectionTrait {
            fn with(mut self, include: IncludeParam) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected> {
                match include { #(#include_match_arms,)* }
                self
            }
            fn include(mut self, includes: Vec<IncludeParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected> {
                for inc in includes { match inc { #(#include_match_arms,)* } }
                self
            }
        }

        pub trait SelectUniqueIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>;
        }
        impl<'a, C> SelectUniqueIncludeExt<'a, C> for caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>
        where C: sea_orm::ConnectionTrait {
            fn with(mut self, include: IncludeParam) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected> {
                match include { #(#include_match_arms,)* }
                self
            }
            fn include(mut self, includes: Vec<IncludeParam>) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected> {
                for inc in includes { match inc { #(#include_match_arms,)* } }
                self
            }
        }

        pub trait SelectFirstIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>;
        }
        impl<'a, C> SelectFirstIncludeExt<'a, C> for caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>
        where C: sea_orm::ConnectionTrait {
            fn with(mut self, include: IncludeParam) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected> {
                match include { #(#include_match_arms,)* }
                self
            }
            fn include(mut self, includes: Vec<IncludeParam>) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected> {
                for inc in includes { match inc { #(#include_match_arms,)* } }
                self
            }
        }

        // Allow using UniqueWhereParam directly as a cursor argument on ManyQueryBuilder
        impl From<UniqueWhereParam> for (sea_query::SimpleExpr, sea_orm::Value) {
            fn from(value: UniqueWhereParam) -> (sea_query::SimpleExpr, sea_orm::Value) {
                use sea_orm::IntoSimpleExpr;
                match value {
                    #(
                        UniqueWhereParam::#unique_where_equals_variants(value) => {
                            let expr = <Entity as EntityTrait>::Column::#unique_where_equals_columns.into_simple_expr();
                            (expr, sea_orm::Value::from(value))
                        }
                    ),*
                }
            }
        }

        #[derive(Debug, Clone)]
        pub enum GroupByOrderByParam {
            #(#group_by_order_by_field_variants,)*
        }

        

        #(#field_ops)*

        // Typed conversion of WhereParam list to Filters (no string parsing)
        #[allow(dead_code)]
        pub fn where_params_to_filters(params: Vec<WhereParam>) -> Vec<caustics::Filter> {
            let mut out = Vec::with_capacity(params.len());
            for p in params {
                let filter = match p {
                    #(#filter_conversion_match_arms,)*
                    // Ignore logical and relation conditions here; those are handled elsewhere
                    WhereParam::And(_) | WhereParam::Or(_) | WhereParam::Not(_) | WhereParam::RelationCondition(_) => continue,
                    // Ignore string mode variants (they affect query mode, not a field filter)
                    _ => continue,
                };
                out.push(filter);
            }
            out
        }

        impl MergeInto<ActiveModel> for SetParam {
            fn merge_into(&self, model: &mut ActiveModel) {
                match self {
                    #(#match_arms,)*
                    #(#atomic_match_arms,)*
                    #(#relation_disconnect_match_arms,)*
                    _ => {
                        // Relation SetParam values are handled in into_active_model, not here
                        // This prevents infinite recursion
                    }
                }
            }
        }

        impl caustics::SetParamInfo for SetParam {
            fn is_has_many_set_operation(&self) -> bool {
                match self {
                    #(#has_many_set_match_arms,)*
                    _ => false,
                }
            }
            
            fn extract_relation_name(&self) -> Option<&'static str> {
                match self {
                    #(#relation_name_match_arms,)*
                    _ => None,
                }
            }
            
            fn extract_target_ids(&self) -> Vec<sea_orm::Value> {
                match self {
                    #(#target_ids_match_arms,)*
                    _ => Vec::new(),
                }
            }
        }

        #(#where_match_arms)*

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

        // Typed aggregate selection enums
        #[derive(Debug, Clone)]
        pub enum SumSelect { #( #sum_select_variants ),* }
        #[derive(Debug, Clone)]
        pub enum AvgSelect { #( #avg_select_variants ),* }
        #[derive(Debug, Clone)]
        pub enum MinSelect { #( #min_select_variants ),* }
        #[derive(Debug, Clone)]
        pub enum MaxSelect { #( #max_select_variants ),* }

        impl SumSelect { fn to_expr(&self) -> sea_query::SimpleExpr { match self { #( SumSelect::#sum_select_variants => <Entity as EntityTrait>::Column::#sum_select_variants.into_simple_expr(), )* } } }
        impl AvgSelect { fn to_expr(&self) -> sea_query::SimpleExpr { match self { #( AvgSelect::#avg_select_variants => <Entity as EntityTrait>::Column::#avg_select_variants.into_simple_expr(), )* } } }
        impl MinSelect { fn to_expr(&self) -> sea_query::SimpleExpr { match self { #( MinSelect::#min_select_variants => <Entity as EntityTrait>::Column::#min_select_variants.into_simple_expr(), )* } } }
        impl MaxSelect { fn to_expr(&self) -> sea_query::SimpleExpr { match self { #( MaxSelect::#max_select_variants => <Entity as EntityTrait>::Column::#max_select_variants.into_simple_expr(), )* } } }

        // Allow using typed enums anywhere a column expression is expected
        impl sea_orm::IntoSimpleExpr for SumSelect { fn into_simple_expr(self) -> sea_query::SimpleExpr { self.to_expr() } }
        impl sea_orm::IntoSimpleExpr for AvgSelect { fn into_simple_expr(self) -> sea_query::SimpleExpr { self.to_expr() } }
        impl sea_orm::IntoSimpleExpr for MinSelect { fn into_simple_expr(self) -> sea_query::SimpleExpr { self.to_expr() } }
        impl sea_orm::IntoSimpleExpr for MaxSelect { fn into_simple_expr(self) -> sea_query::SimpleExpr { self.to_expr() } }

        // Provide an entity-specific extension trait on ManyQueryBuilder to accept a typed UniqueWhereParam as cursor
        pub trait ManyCursorExt<'a, C: sea_orm::ConnectionTrait> {
            fn cursor(self, unique: UniqueWhereParam) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> ManyCursorExt<'a, C>
            for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        {
            fn cursor(mut self, unique: UniqueWhereParam) -> Self {
                use sea_orm::IntoSimpleExpr;
                match unique {
                    #(#unique_cursor_match_arms)*
                }
            }
        }

        // Contribute to prelude module for this entity
        pub mod prelude {
            pub use super::ManyCursorExt;
            pub use super::DistinctFieldsExt;
            pub use super::SelectManyDistinctFieldsExt;
            pub use super::AggregateSelectorExt;
            pub use super::GroupBySelectorExt;
            pub use super::GroupByHavingAggExt;
            pub use super::GroupByAggExt;
            pub use super::AggregateAggExt;
            pub use super::ManySelectExt;
            pub use super::UniqueSelectExt;
            pub use super::FirstSelectExt;
            pub use super::SelectManyIncludeExt;
            pub use super::SelectUniqueIncludeExt;
            pub use super::SelectFirstIncludeExt;
        }

        // PCR-style aggregate selector facade on AggregateQueryBuilder
        pub trait AggregateSelectorExt<'a, C: sea_orm::ConnectionTrait> {
            fn _count(self) -> caustics::AggregateQueryBuilder<'a, C, Entity>;
            fn _avg(self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity>;
            fn _sum(self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity>;
            fn _min(self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity>;
            fn _max(self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity>;
        }

        impl<'a, C: sea_orm::ConnectionTrait> AggregateSelectorExt<'a, C>
            for caustics::AggregateQueryBuilder<'a, C, Entity>
        {
            fn _count(mut self) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                self = self.select_count();
                self
            }

            fn _avg(mut self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_avg_typed(AvgSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_avg"));
                        },)*
                    }
                }
                self
            }

            fn _sum(mut self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_sum_typed(SumSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_sum"));
                        },)*
                    }
                }
                self
            }

            fn _min(mut self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_min_typed(MinSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_min"));
                        },)*
                    }
                }
                self
            }

            fn _max(mut self, fields: Vec<ScalarField>) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_max_typed(MaxSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_max"));
                        },)*
                    }
                }
                self
            }
        }

        // PCR-style aggregate selector facade on GroupByQueryBuilder
        pub trait GroupBySelectorExt<'a, C: sea_orm::ConnectionTrait> {
            fn _count(self) -> caustics::GroupByQueryBuilder<'a, C, Entity>;
            fn _avg(self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity>;
            fn _sum(self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity>;
            fn _min(self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity>;
            fn _max(self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity>;
        }

        impl<'a, C: sea_orm::ConnectionTrait> GroupBySelectorExt<'a, C>
            for caustics::GroupByQueryBuilder<'a, C, Entity>
        {
            fn _count(mut self) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                self = self.select_count("_count");
                self
            }

            fn _avg(mut self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_avg_typed(AvgSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_avg"));
                        },)*
                    }
                }
                self
            }

            fn _sum(mut self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_sum_typed(SumSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_sum"));
                        },)*
                    }
                }
                self
            }

            fn _min(mut self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_min_typed(MinSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_min"));
                        },)*
                    }
                }
                self
            }

            fn _max(mut self, fields: Vec<ScalarField>) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                for f in fields {
                    match f {
                        #(ScalarField::#group_by_field_variants => {
                            self = self.select_max_typed(MaxSelect::#group_by_field_variants, concat!(stringify!(#group_by_field_variants), "_max"));
                        },)*
                    }
                }
                self
            }
        }

        // Extend GroupByQueryBuilder with typed aggregate selectors via a local trait
        pub trait GroupByAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn select_sum_typed(self, field: SumSelect, alias: &'static str) -> Self;
            fn select_avg_typed(self, field: AvgSelect, alias: &'static str) -> Self;
            fn select_min_typed(self, field: MinSelect, alias: &'static str) -> Self;
            fn select_max_typed(self, field: MaxSelect, alias: &'static str) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> GroupByAggExt<'a, C> for caustics::GroupByQueryBuilder<'a, C, Entity> {
            fn select_sum_typed(mut self, field: SumSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::sum(field.to_expr())), alias)); self }
            fn select_avg_typed(mut self, field: AvgSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::avg(field.to_expr())), alias)); self }
            fn select_min_typed(mut self, field: MinSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::min(field.to_expr())), alias)); self }
            fn select_max_typed(mut self, field: MaxSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::max(field.to_expr())), alias)); self }
        }

        // Typed group-by order by aggregate outputs
        #[derive(Debug, Clone)]
        pub enum GroupByAggOrderParam {
            Count(caustics::SortOrder),
            Sum(SumSelect, caustics::SortOrder),
            Avg(AvgSelect, caustics::SortOrder),
            Min(MinSelect, caustics::SortOrder),
            Max(MaxSelect, caustics::SortOrder),
        }

        // Typed aggregate HAVING helpers
        pub trait GroupByHavingAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn having_sum_gt<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;
            fn having_sum_gte<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;
            fn having_sum_lt<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;
            fn having_sum_lte<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;
            fn having_sum_eq<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;
            fn having_sum_ne<V: Into<sea_orm::Value>>(self, field: SumSelect, value: V) -> Self;

            fn having_avg_gt<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;
            fn having_avg_gte<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;
            fn having_avg_lt<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;
            fn having_avg_lte<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;
            fn having_avg_eq<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;
            fn having_avg_ne<V: Into<sea_orm::Value>>(self, field: AvgSelect, value: V) -> Self;

            fn having_min_gt<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;
            fn having_min_gte<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;
            fn having_min_lt<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;
            fn having_min_lte<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;
            fn having_min_eq<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;
            fn having_min_ne<V: Into<sea_orm::Value>>(self, field: MinSelect, value: V) -> Self;

            fn having_max_gt<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
            fn having_max_gte<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
            fn having_max_lt<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
            fn having_max_lte<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
            fn having_max_eq<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
            fn having_max_ne<V: Into<sea_orm::Value>>(self, field: MaxSelect, value: V) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> GroupByHavingAggExt<'a, C> for caustics::GroupByQueryBuilder<'a, C, Entity> {
            fn having_sum_gt<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_sum_gte<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_sum_lt<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_sum_lte<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_sum_eq<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_sum_ne<V: Into<sea_orm::Value>>(mut self, field: SumSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_avg_gt<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_avg_gte<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_avg_lt<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_avg_lte<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_avg_eq<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_avg_ne<V: Into<sea_orm::Value>>(mut self, field: AvgSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_min_gt<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_min_gte<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_min_lt<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_min_lte<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_min_eq<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_min_ne<V: Into<sea_orm::Value>>(mut self, field: MinSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_max_gt<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_max_gte<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_max_lt<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_max_lte<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_max_eq<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_max_ne<V: Into<sea_orm::Value>>(mut self, field: MaxSelect, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }
        }

        // Add typed aggregate selection for non-group aggregate queries via a trait
        pub trait AggregateAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn select_sum_typed(self, field: SumSelect, alias: &'static str) -> Self;
            fn select_avg_typed(self, field: AvgSelect, alias: &'static str) -> Self;
            fn select_min_typed(self, field: MinSelect, alias: &'static str) -> Self;
            fn select_max_typed(self, field: MaxSelect, alias: &'static str) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> AggregateAggExt<'a, C> for caustics::AggregateQueryBuilder<'a, C, Entity> {
            fn select_sum_typed(mut self, field: SumSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::sum(field.to_expr())), alias, "sum")); self }
            fn select_avg_typed(mut self, field: AvgSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::avg(field.to_expr())), alias, "avg")); self }
            fn select_min_typed(mut self, field: MinSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::min(field.to_expr())), alias, "min")); self }
            fn select_max_typed(mut self, field: MaxSelect, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::max(field.to_expr())), alias, "max")); self }
        }

        // Removed inherent impl to avoid E0116 in downstream crates; use GroupByAggExt instead

        pub struct Create {
            #(#required_struct_fields,)*
            #(#foreign_key_relation_fields,)*
            pub _params: Vec<SetParam>,
        }

        impl Create {
            fn into_active_model<C: sea_orm::ConnectionTrait>(mut self) -> (ActiveModel, Vec<caustics::DeferredLookup>) {
                let mut model = ActiveModel::new();
                let mut deferred_lookups = Vec::new();

                #(#required_assigns)*
                #(#foreign_key_assigns)*

                // Process SetParam values
                for param in self._params {
                    match param {
                        #(#relation_connect_deferred_match_arms,)*
                        #(#relation_disconnect_match_arms,)*
                        other => {
                            // For non-relation SetParam values, use the normal merge_into
                            other.merge_into(&mut model);
                        }
                    }
                }
                (model, deferred_lookups)
            }
        }

        #model_with_relations_impl
        #relation_metadata_impl

        // PCR-style typed distinct extension for ManyQueryBuilder at module scope
        pub trait DistinctFieldsExt<'a, C: sea_orm::ConnectionTrait> {
            fn distinct(self, fields: Vec<ScalarField>) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> DistinctFieldsExt<'a, C>
            for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        {
            fn distinct(mut self, fields: Vec<ScalarField>) -> Self {
                let mut exprs: Vec<SimpleExpr> = Vec::with_capacity(fields.len());
                for f in fields {
                    let e = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    exprs.push(e);
                }
                self.distinct_on(exprs)
            }
        }

        // Expose distinct(fields) on SelectManyQueryBuilder as well
        pub trait SelectManyDistinctFieldsExt<'a, C: sea_orm::ConnectionTrait> {
            fn distinct(self, fields: Vec<ScalarField>) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> SelectManyDistinctFieldsExt<'a, C>
            for caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>
        {
            fn distinct(mut self, fields: Vec<ScalarField>) -> Self {
                let mut exprs: Vec<SimpleExpr> = Vec::with_capacity(fields.len());
                for f in fields {
                    let e = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    exprs.push(e);
                }
                self.distinct_on_fields = Some(exprs);
                self.is_distinct = true;
                self
            }
        }

        impl<'a, C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait> EntityClient<'a, C> {
            pub fn new(conn: &'a C, database_backend: sea_orm::DatabaseBackend) -> Self {
                Self { conn, database_backend }
            }

            pub fn find_unique(&self, condition: UniqueWhereParam) -> caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let registry = __caustics_fetch_registry();
                caustics::UniqueQueryBuilder {
                    query: <Entity as EntityTrait>::find().filter::<Condition>(condition.clone().into()),
                    conn: self.conn,
                    relations_to_fetch: vec![],
                    registry,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn find_first(&self, conditions: Vec<WhereParam>) -> caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let registry = __caustics_fetch_registry();
                let query = <Entity as EntityTrait>::find().filter::<Condition>(where_params_to_condition(conditions, self.database_backend));
                caustics::FirstQueryBuilder {
                    query,
                    conn: self.conn,
                    relations_to_fetch: vec![],
                    registry,
                    database_backend: self.database_backend,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn find_many(&self, conditions: Vec<WhereParam>) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let registry = __caustics_fetch_registry();
                let query = <Entity as EntityTrait>::find().filter::<Condition>(where_params_to_condition(conditions, self.database_backend));
                caustics::ManyQueryBuilder {
                    query,
                    conn: self.conn,
                    relations_to_fetch: vec![],
                    registry,
                    database_backend: self.database_backend,
                    reverse_order: false,
                    pending_order_bys: Vec::new(),
                    cursor: None,
                    is_distinct: false,
                    distinct_on_fields: None,
                    skip_is_negative: false,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn distinct(
                &self,
                mut builder: caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>,
                fields: Vec<DistinctFieldParam>,
            ) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let mut exprs: Vec<SimpleExpr> = Vec::with_capacity(fields.len());
                for f in fields {
                    let e = match f {
                        #(DistinctFieldParam::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    exprs.push(e);
                }
                builder.distinct_on(exprs)
            }

            // NOTE: PCR-style aggregation and distinct builder facades will be added incrementally

            

            pub fn count(&self, conditions: Vec<WhereParam>) -> caustics::CountQueryBuilder<'a, C, Entity> {
                let condition = where_params_to_condition(conditions, self.database_backend);
                caustics::CountQueryBuilder {
                    condition,
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn aggregate(&self, conditions: Vec<WhereParam>) -> caustics::AggregateQueryBuilder<'a, C, Entity> {
                let condition = where_params_to_condition(conditions, self.database_backend);
                caustics::AggregateQueryBuilder {
                    condition,
                    conn: self.conn,
                    selections: caustics::query_builders::aggregate::AggregateSelections::default(),
                    aggregates: Vec::new(),
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn group_by(
                &self,
                by: Vec<GroupByFieldParam>,
                r#where: Vec<WhereParam>,
                order_by: Vec<(GroupByFieldParam, caustics::SortOrder)>,
                take: Option<i64>,
                skip: Option<i64>,
                having: Option<sea_orm::sea_query::Condition>,
            ) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                use sea_orm::IntoSimpleExpr;
                let condition = where_params_to_condition(r#where, self.database_backend);
                let mut exprs: Vec<SimpleExpr> = Vec::with_capacity(by.len());
                let mut group_cols: Vec<String> = Vec::with_capacity(by.len());
                for b in by {
                    let e = match b {
                        #(GroupByFieldParam::#group_by_field_variants => {
                            group_cols.push(stringify!(#group_by_field_variants).to_string());
                            <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr()
                        },)*
                    };
                    exprs.push(e);
                }
                let mut builder = caustics::GroupByQueryBuilder {
                    condition,
                    conn: self.conn,
                    group_by_exprs: exprs,
                    group_by_columns: group_cols,
                    having: Vec::new(),
                    having_condition: None,
                    order_by: Vec::new(),
                    take: None,
                    skip: None,
                    aggregates: Vec::new(),
                    _phantom: std::marker::PhantomData,
                };
                for (field, dir) in order_by {
                    let expr = match field {
                        #(GroupByFieldParam::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                    builder.order_by.push((expr, ord));
                }
                if let Some(n) = take { builder.take = Some(if n < 0 { 0 } else { n as u64 }); }
                if let Some(n) = skip { builder.skip = Some(if n < 0 { 0 } else { n as u64 }); }
                if let Some(cond) = having { builder.having_condition = Some(cond); }
                builder
            }

            pub fn group_by_order_by(
                &self,
                builder: caustics::GroupByQueryBuilder<'a, C, Entity>,
                order: Vec<GroupByOrderByParam>,
            ) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                let mut pairs: Vec<(sea_query::SimpleExpr, sea_orm::Order)> = Vec::with_capacity(order.len());
                for o in order {
                    let pair = match o {
                        #(#group_by_order_by_match_arms,)*
                    };
                    pairs.push(pair);
                }
                builder.order_by_pairs(pairs)
            }

            pub fn group_by_order_by_aggregates(
                &self,
                builder: caustics::GroupByQueryBuilder<'a, C, Entity>,
                order: Vec<GroupByAggOrderParam>,
            ) -> caustics::GroupByQueryBuilder<'a, C, Entity> {
                use sea_orm::sea_query::{Expr, Func, SimpleExpr};
                let mut pairs: Vec<(SimpleExpr, sea_orm::Order)> = Vec::with_capacity(order.len());
                for o in order {
                    match o {
                        GroupByAggOrderParam::Count(dir) => {
                            let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                            pairs.push((Expr::cust("COUNT(*)"), ord));
                        }
                        GroupByAggOrderParam::Sum(field, dir) => {
                            let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                            pairs.push((SimpleExpr::FunctionCall(Func::sum(field.to_expr())), ord));
                        }
                        GroupByAggOrderParam::Avg(field, dir) => {
                            let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                            pairs.push((SimpleExpr::FunctionCall(Func::avg(field.to_expr())), ord));
                        }
                        GroupByAggOrderParam::Min(field, dir) => {
                            let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                            pairs.push((SimpleExpr::FunctionCall(Func::min(field.to_expr())), ord));
                        }
                        GroupByAggOrderParam::Max(field, dir) => {
                            let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                            pairs.push((SimpleExpr::FunctionCall(Func::max(field.to_expr())), ord));
                        }
                    }
                }
                builder.order_by_pairs(pairs)
            }

            

            pub fn create(&self, #(#required_fn_args,)* #(#foreign_key_relation_args,)* _params: Vec<SetParam>) -> caustics::CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations> {
                let create = Create {
                    #(#required_inits,)*
                    #(#foreign_key_relation_inits,)*
                    _params,
                };
                let (model, deferred_lookups) = create.into_active_model::<C>();
                caustics::CreateQueryBuilder {
                    model,
                    conn: self.conn,
                    deferred_lookups,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn update(&self, condition: UniqueWhereParam, changes: Vec<SetParam>) -> caustics::UnifiedUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam>
            where
                C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
                ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model>
                    + caustics::HasRelationMetadata<ModelWithRelations>
                    + 'static,
            {
                let cond: Condition = condition.into();
                let cond_arc = Arc::new(cond.clone());
                let resolver: Box<
                    dyn for<'b> Fn(
                            &'b C,
                        ) -> std::pin::Pin<
                            Box<
                                dyn std::future::Future<Output = Result<sea_orm::Value, sea_orm::DbErr>>
                                    + Send
                                    + 'b,
                            >,
                        > + Send,
                > = Box::new({
                    let cond_arc_outer = Arc::clone(&cond_arc);
                    move |conn: &C| {
                        // Clone the Arc inside the Fn each call to preserve Fn semantics
                        let cond_arc_inner = Arc::clone(&cond_arc_outer);
                        let fut = async move {
                            use sea_orm::{EntityTrait, QueryFilter};
                            let cond_local = (*cond_arc_inner).clone();
                            let found = <Entity as EntityTrait>::find()
                                .filter::<Condition>(cond_local)
                                .one(conn)
                                .await?;
                            if let Some(model) = found {
                                let id_val: i32 = model.#current_primary_key_ident;
                                Ok(sea_orm::Value::Int(Some(id_val)))
                            } else {
                                Err(sea_orm::DbErr::RecordNotFound("No record matched for has_many set".to_string()))
                            }
                        };
                        Box::pin(fut)
                    }
                });
                let has_many = changes.iter().any(|c| <SetParam as caustics::SetParamInfo>::is_has_many_set_operation(c));
                if has_many {
                    caustics::UnifiedUpdateQueryBuilder::Relations(caustics::HasManySetUpdateQueryBuilder {
                        condition: cond,
                        changes,
                        conn: self.conn,
                        entity_id_resolver: Some(resolver),
                        _phantom: std::marker::PhantomData,
                    })
                } else {
                    caustics::UnifiedUpdateQueryBuilder::Scalar(caustics::UpdateQueryBuilder {
                        condition: cond,
                        changes,
                        conn: self.conn,
                        _phantom: std::marker::PhantomData,
                    })
                }
            }

            pub fn delete(&self, condition: UniqueWhereParam) -> caustics::DeleteQueryBuilder<'a, C, Entity, ModelWithRelations> {
                caustics::DeleteQueryBuilder {
                    condition: condition.into(),
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn delete_many(&self, conditions: Vec<WhereParam>) -> caustics::DeleteManyQueryBuilder<'a, C, Entity> {
                let cond = where_params_to_condition(conditions, self.database_backend);
                caustics::DeleteManyQueryBuilder {
                    condition: cond,
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn upsert(&self, condition: UniqueWhereParam, create: Create, update: Vec<SetParam>) -> caustics::UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam> {
                let (model, deferred_lookups) = create.into_active_model::<C>();
                caustics::UpsertQueryBuilder {
                    condition: condition.into(),
                    create: (model, deferred_lookups),
                    update,
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
                }
            }

        pub async fn _batch<Container>(
            &self,
            queries: Container,
        ) -> Result<Container::ReturnType, sea_orm::DbErr>
        where
            Entity: sea_orm::EntityTrait,
            ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model>,
            SetParam: caustics::MergeInto<ActiveModel>,
            <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
            Container: caustics::BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam>,
        {
            caustics::batch::<C, Entity, ActiveModel, ModelWithRelations, SetParam, Container>(
                queries,
                self.conn,
            )
            .await
        }


        }

        // Include the generated relation submodules
        #relation_submodules

        // Generate column_from_str function
        #column_from_str_fn

        // --- Begin entity fetcher and registry generation ---
        pub struct EntityFetcherImpl;

        impl<C: sea_orm::ConnectionTrait> caustics::EntityFetcher<C> for EntityFetcherImpl {
            fn fetch_by_foreign_key<'a>(
                &'a self,
                conn: &'a C,
                foreign_key_value: Option<i32>,
                foreign_key_column: &'a str,
                target_entity: &'a str,
                relation_name: &'a str,
                filter: &'a caustics::RelationFilter,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>, sea_orm::DbErr>> + Send + 'a>> {
                Box::pin(async move {
                    match relation_name {
                        #(
                        #relation_names_snake_lits => { #relation_fetcher_bodies }
                        )*
                        _ => Err(sea_orm::DbErr::Custom(format!(
                            "Unknown relation '{}': ensure relation name matches generated metadata",
                            relation_name
                        ))),
                    }
                })
            }
        }

        // Implement FromModel<Model> for Model
        impl FromModel<Model> for Model {
            fn from_model(m: Model) -> Self {
                m
            }
        }

        // Implement ActiveModelBehavior for ActiveModel
        impl sea_orm::ActiveModelBehavior for ActiveModel {}

    };

    TokenStream::from(expanded)
}

fn extract_relations(
    relation_ast: &DeriveInput,
    model_fields: &[&syn::Field],
    current_table_name: &str,
) -> Vec<Relation> {
    let mut relations = Vec::new();

    if let syn::Data::Enum(data_enum) = &relation_ast.data {
        for variant in &data_enum.variants {
            let mut foreign_key_field = None;
            let mut foreign_key_type = None;
            let mut relation_name = None;
            let mut relation_target = None;
            let mut relation_kind = None;
            let mut is_nullable = false;
            let mut foreign_key_column = None;
            let mut primary_key_field = None;

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
                                                foreign_key_field = Some(snake_case_name.clone());

                                                // Find the corresponding field in the model to get its type
                                                if let Some(field) = model_fields.iter().find(|f| {
                                                    f.ident.as_ref().unwrap().to_string() == snake_case_name
                                                }) {
                                                    foreign_key_type = Some(field.ty.clone());
                                                }
                                            }
                                        }
                                    } else if nv.path.is_ident("to") {
                                        if let syn::Expr::Lit(syn::ExprLit {
                                            lit: syn::Lit::Str(lit),
                                            ..
                                        }) = &nv.value
                                        {
                                            // Extract foreign key column name from "Entity::Column::FieldName"
                                            let column_str = lit.value();
                                            if let Some(field_name) = column_str.split("::").last() {
                                                foreign_key_column = Some(field_name.to_string());
                                            }
                                        }
                                    } else if nv.path.is_ident("nullable") {
                                        is_nullable = true;
                                    } else if nv.path.is_ident("column") {
                                        if let syn::Expr::Lit(syn::ExprLit {
                                            lit: syn::Lit::Str(lit),
                                            ..
                                        }) = &nv.value
                                        {
                                            foreign_key_column = Some(lit.value());
                                        }
                                    } else if nv.path.is_ident("primary_key") {
                                        if let syn::Expr::Lit(syn::ExprLit {
                                            lit: syn::Lit::Str(lit),
                                            ..
                                        }) = &nv.value
                                        {
                                            primary_key_field = Some(lit.value());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Only add the relation if we have all the required information
            if let (Some(name), Some(target), Some(kind)) =
                (relation_name, relation_target, relation_kind)
            {
                // Construct the target unique param path
                let target_unique_param = if foreign_key_field.is_some() {
                    let mut unique_param_path = target.clone();
                    unique_param_path.segments.push(syn::PathSegment {
                        ident: syn::Ident::new("UniqueWhereParam", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None,
                    });
                    Some(unique_param_path)
                } else {
                    None
                };

                // Check if the foreign key field is nullable by examining its type
                if let Some(fk_field_name) = &foreign_key_field {
                    if let Some(field) = model_fields
                        .iter()
                        .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
                    {
                        if is_option(&field.ty) {
                            is_nullable = true;
                        }
                    }
                }

                // Extract target table name from target entity path
                let target_table_name = if let Some(last_segment) = target.segments.last() {
                    // Use the relation name (which is typically plural) instead of the entity name
                    // This is more likely to match the actual table name
                    name.to_snake_case()
                } else {
                    "unknown".to_string()
                };

                relations.push(Relation {
                    name,
                    target,
                    kind,
                    foreign_key_field,
                    foreign_key_type,
                    target_unique_param,
                    is_nullable,
                    foreign_key_column,
                    primary_key_field,
                    current_table_name: Some(current_table_name.to_string()),
                    target_table_name: Some(target_table_name),
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
        let relation_name_ident = format_ident!("{}", relation_name.to_lowercase());
        let relation_name_lower = relation_name.to_lowercase();
        let relation_name_lower_ident = format_ident!("{}", relation_name_lower);
        let relation_name_str = relation_name.to_snake_case();
        let relation_name_lit =
            syn::LitStr::new(&relation_name_str, proc_macro2::Span::call_site());
        let target = &relation.target;
        let connect_variant = format_ident!("Connect{}", relation.name.to_pascal_case());
        let disconnect_variant = format_ident!("Disconnect{}", relation.name.to_pascal_case());
        let set_variant = format_ident!("Set{}", relation.name.to_pascal_case());

        // Generate conditional functions
        let set_fn = if matches!(relation.kind, RelationKind::HasMany) {
            quote! {
                pub fn set(where_params: Vec<super::#target::UniqueWhereParam>) -> super::SetParam {
                    super::SetParam::#set_variant(where_params)
                }
            }
        } else {
            quote! {}
        };

        // Generate disconnect only for optional belongs_to (nullable FK on current entity)
        let disconnect_fn = if matches!(relation.kind, RelationKind::BelongsTo)
            && relation.foreign_key_field.is_some()
        {
            let fk_field_name = relation.foreign_key_field.as_ref().unwrap();
            let is_optional = if let Some(field) = fields
                .iter()
                .find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name)
            {
                is_option(&field.ty)
            } else {
                false
            };
            if is_optional {
                quote! {
                    pub fn disconnect() -> super::SetParam {
                        super::SetParam::#disconnect_variant
                    }
                }
            } else {
                quote! {}
            }
        } else {
            quote! {}
        };

        // Get foreign key column information from relation metadata
        let foreign_key_column_ident = if let Some(fk_col) = &relation.foreign_key_column {
            format_ident!("{}", fk_col)
        } else {
            format_ident!("Id") // fallback
        };

        let submodule = quote! {
            pub mod #relation_name_ident {
                use super::*;

                // Typed conversion function for relation filters (no string parsing)
                fn convert_where_param_to_filter_generic(filter: super::#target::WhereParam) -> caustics::Filter {
                    // Delegate to the target module's typed converter
                    super::#target::where_params_to_filters(vec![filter])
                        .into_iter()
                        .next()
                        .unwrap_or(caustics::Filter { field: String::new(), operation: caustics::FieldOp::IsNull })
                }

                // Basic relation functions
                pub fn fetch() -> super::RelationFilter {
                    super::RelationFilter {
                        relation: #relation_name_lit,
                        filters: vec![],
                        nested_select_aliases: None,
                        nested_includes: vec![],
                        take: None,
                        skip: None,
                        order_by: vec![],
                        cursor_id: None,
                        include_count: false,
                    }
                }

                pub fn fetch_with_includes(includes: Vec<super::#target::RelationFilter>) -> super::RelationFilter {
                    super::RelationFilter {
                        relation: #relation_name_lit,
                        filters: vec![],
                        nested_select_aliases: None,
                        nested_includes: includes.into_iter().map(Into::into).collect(),
                        take: None,
                        skip: None,
                        order_by: vec![],
                        cursor_id: None,
                        include_count: false,
                    }
                }

                pub fn fetch_with_select(select_aliases: Vec<&str>) -> super::RelationFilter {
                    super::RelationFilter {
                        relation: #relation_name_lit,
                        filters: vec![],
                        nested_select_aliases: Some(select_aliases.into_iter().map(|s| s.to_string()).collect()),
                        nested_includes: vec![],
                        take: None,
                        skip: None,
                        order_by: vec![],
                        cursor_id: None,
                        include_count: false,
                    }
                }

                // PCR-aligned typed helpers
                pub fn fetch_with_select_params(params: Vec<super::#target::SelectParam>) -> super::RelationFilter {
                    let aliases: Vec<String> = super::#target::select_params_to_aliases(params);
                    super::RelationFilter {
                        relation: #relation_name_lit,
                        filters: vec![],
                        nested_select_aliases: Some(aliases),
                        nested_includes: vec![],
                        take: None,
                        skip: None,
                        order_by: vec![],
                        cursor_id: None,
                        include_count: false,
                    }
                }

                pub fn with_select(params: Vec<super::#target::SelectParam>) -> super::RelationFilter {
                    fetch_with_select_params(params)
                }

                pub fn with(include: super::#target::RelationFilter) -> super::RelationFilter {
                    fetch_with_includes(vec![include])
                }

                pub fn with_includes(includes: Vec<super::#target::RelationFilter>) -> super::RelationFilter {
                    fetch_with_includes(includes)
                }

                pub fn take(limit: i64) -> super::RelationFilter { let mut f = fetch(); f.take = Some(limit); f }
                pub fn skip(offset: i64) -> super::RelationFilter { let mut f = fetch(); f.skip = Some(offset); f }

                pub fn with_order(params: Vec<super::OrderByParam>) -> super::RelationFilter {
                    let mut f = fetch();
                    for p in params {
                        let (col, ord): (<Entity as EntityTrait>::Column, sea_orm::Order) = p.into();
                        // Map column to snake_case alias string via debug
                        let name = format!("{:?}", col).to_string().to_snake_case();
                        f.order_by.push((name, match ord { sea_orm::Order::Asc => caustics::SortOrder::Asc, _ => caustics::SortOrder::Desc }));
                    }
                    f
                }

                pub fn with_cursor(id: i32) -> super::RelationFilter { let mut f = fetch(); f.cursor_id = Some(id); f }
                pub fn with_count() -> super::RelationFilter { let mut f = fetch(); f.include_count = true; f }

                pub fn connect(where_param: super::#target::UniqueWhereParam) -> super::SetParam {
                    super::SetParam::#connect_variant(where_param)
                }

                #set_fn
                #disconnect_fn

                // Advanced relation operations for filtering
                pub fn some(filters: Vec<super::#target::WhereParam>) -> super::WhereParam {
                    // Convert WhereParam filters to Filter format for relation conditions
                    let mut relation_filters = Vec::new();
                    for filter in filters {
                        // Convert the WhereParam to a Filter by extracting field name and value
                        let field_info = convert_where_param_to_filter_generic(filter);
                        relation_filters.push(caustics::Filter {
                            field: field_info.field,
                            operation: field_info.operation,
                        });
                    }

                    super::WhereParam::RelationCondition(caustics::RelationCondition::some(#relation_name_lit, relation_filters))
                }

                pub fn every(filters: Vec<super::#target::WhereParam>) -> super::WhereParam {
                    // Convert WhereParam filters to Filter format for relation conditions
                    let mut relation_filters = Vec::new();
                    for filter in filters {
                        // Convert the WhereParam to a Filter by extracting field name and value
                        let field_info = convert_where_param_to_filter_generic(filter);
                        relation_filters.push(caustics::Filter {
                            field: field_info.field,
                            operation: field_info.operation,
                        });
                    }
                    super::WhereParam::RelationCondition(caustics::RelationCondition::every(#relation_name_lit, relation_filters))
                }

                pub fn none(filters: Vec<super::#target::WhereParam>) -> super::WhereParam {
                    // Convert WhereParam filters to Filter format for relation conditions
                    let mut relation_filters = Vec::new();
                    for filter in filters {
                        // Convert the WhereParam to a Filter by extracting field name and value
                        let field_info = convert_where_param_to_filter_generic(filter);
                        relation_filters.push(caustics::Filter {
                            field: field_info.field,
                            operation: field_info.operation,
                        });
                    }
                    super::WhereParam::RelationCondition(caustics::RelationCondition::none(#relation_name_lit, relation_filters))
                }
            }
        };
        submodules.push(submodule);
    }

    quote! {
        #(#submodules)*
    }
}

/// Extract table name from entity attributes
fn extract_table_name(model_ast: &DeriveInput) -> String {
    for attr in &model_ast.attrs {
        if let syn::Meta::List(meta) = &attr.meta {
            if meta.path.is_ident("sea_orm") {
                if let Ok(nested) = meta.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let syn::Meta::NameValue(nv) = &meta {
                            if nv.path.is_ident("table_name") {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit),
                                    ..
                                }) = &nv.value
                                {
                                    return lit.value();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Default to snake_case of the struct name
    model_ast.ident.to_string().to_snake_case()
}
