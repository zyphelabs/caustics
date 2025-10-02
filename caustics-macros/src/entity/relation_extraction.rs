use crate::common::is_option;
use crate::entity::types::{Relation, RelationKind};
use heck::ToSnakeCase;
use syn::DeriveInput;

#[allow(clippy::cmp_owned)]
pub fn extract_relations(
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
            let mut target_entity_name = None;

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
                                                    *f.ident.as_ref().unwrap() == snake_case_name
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
                                                // Convert PascalCase to snake_case to match database column names
                                                let snake_case_name = field_name.to_string().to_snake_case();
                                                foreign_key_column = Some(snake_case_name.clone());

                                                // Extract entity name from the "to" attribute path
                                                // Format: "super::entity::Column::FieldName"
                                                // We need to get the entity name (e.g., "Post" from "super::post::Column::Id")
                                                let path_parts: Vec<&str> = column_str.split("::").collect();
                                                if path_parts.len() >= 3 {
                                                    // Get the entity name from the path (e.g., "post" from "super::post::Column::Id")
                                                    let entity_name_lower = path_parts[path_parts.len() - 3].to_lowercase();
                                                    // Convert to PascalCase to match the registry format
                                                    let entity_name = entity_name_lower.chars().next().unwrap().to_uppercase().collect::<String>() + &entity_name_lower[1..];

                                // Store the entity name for runtime resolution
                                // The actual primary key resolution will happen in the generated code
                                // using get_entity_metadata() function
                                target_entity_name = Some(entity_name);

                                // Set primary_key_field to None for runtime resolution
                                // The runtime code will use target_entity_name to look up the actual primary key
                                primary_key_field = None;
                                                }
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
                    target_entity_name,
                    current_table_name: Some(current_table_name.to_string()),
                });
            }
        }
    }

    relations
}
