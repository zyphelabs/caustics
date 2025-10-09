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
            // Collect all sea_orm attributes for this variant
            let mut relation_attrs = Vec::new();
            for attr in &variant.attrs {
                if let syn::Meta::List(meta) = &attr.meta {
                    if meta.path.is_ident("sea_orm") {
                        if let Ok(nested) = meta.parse_args_with(
                            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                        ) {
                            relation_attrs.push(nested);
                        }
                    }
                }
            }

            // Initialize relation with new composite fields
            let mut relation = Relation {
                name: variant.ident.to_string(),
                target: syn::Path {
                    leading_colon: None,
                    segments: syn::punctuated::Punctuated::new(),
                },
                kind: RelationKind::BelongsTo,
                foreign_key_field: None,
                foreign_key_type: None,
                foreign_key_column: None,
                primary_key_field: None,
                foreign_key_fields: Vec::new(),
                foreign_key_types: Vec::new(),
                foreign_key_columns: Vec::new(),
                target_primary_key_fields: Vec::new(),
                target_primary_key_columns: Vec::new(),
                target_unique_param: None,
                is_nullable: false,
                target_entity_name: None,
                current_table_name: Some(current_table_name.to_string()),
                is_composite: false,
                composite_key_mapping: Vec::new(),
            };

            // Process all sea_orm attributes for this variant
            for attrs in relation_attrs {
                let mut belongs_to_target = None;
                let mut from_field = None;
                let mut to_field = None;
                
                for meta in attrs {
                    if let syn::Meta::NameValue(nv) = &meta {
                        match nv.path.get_ident().map(|i| i.to_string()).as_deref() {
                            Some("belongs_to") => {
                                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &nv.value {
                                    belongs_to_target = Some(parse_target_path(&lit.value()));
                                    relation.kind = RelationKind::BelongsTo;
                                }
                            }
                            Some("has_many") => {
                                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &nv.value {
                                    belongs_to_target = Some(parse_target_path(&lit.value()));
                                    relation.kind = RelationKind::HasMany;
                                }
                            }
                            Some("from") => {
                                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &nv.value {
                                    from_field = Some(extract_field_name(&lit.value()));
                                }
                            }
                            Some("to") => {
                                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &nv.value {
                                    to_field = Some(extract_field_name(&lit.value()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                
                // If we found a complete relation (belongs_to or has_many), add it to the composite fields
                if let (Some(from), Some(to), Some(target)) = (from_field, to_field, belongs_to_target) {
                    relation.target = target;
                    // from and to are already Column enum variant names (PascalCase)
                    let from_column_variant = from;
                    let to_column_variant = to;
                    let from_snake = from_column_variant.to_snake_case();
                    let to_snake = to_column_variant.to_snake_case();
                    
                    // For belongs_to: from=FK in current entity, to=PK in target entity
                    // For has_many: from=PK in current entity, to=FK in target entity
                    if relation.kind == RelationKind::BelongsTo {
                        relation.foreign_key_fields.push(from_snake.clone());
                        relation.target_primary_key_fields.push(to_snake.clone());
                        relation.foreign_key_columns.push(from_column_variant.clone());
                        relation.target_primary_key_columns.push(to_column_variant.clone());
                    } else {
                        // For has_many, store in both places for compatibility
                        relation.foreign_key_field = Some(to_snake.clone());
                        relation.foreign_key_column = Some(to_column_variant.clone());
                        relation.primary_key_field = Some(from_snake.clone());
                        // Also populate the vec fields for code generation compatibility
                        relation.foreign_key_fields.push(to_snake.clone());
                        relation.foreign_key_columns.push(to_column_variant.clone());
                        // For has_many, the "to" field is the target's FK, and "from" is current entity's PK
                        relation.target_primary_key_fields.push(from_snake.clone());
                    }
                    
                    relation.composite_key_mapping.push((from_snake, to_snake));
                    // Don't set is_composite for single-field relations
                    // is_composite will be set to true later if we find multiple from/to pairs
                    
                            // Check if the foreign key field is optional
                            let from_field_name = from_column_variant.to_snake_case();
                            if let Some(field) = model_fields.iter().find(|f| {
                                f.ident.as_ref().unwrap().to_string() == from_field_name
                            }) {
                                if is_option(&field.ty) {
                                    relation.is_nullable = true;
                                }
                            }
                }
            }
            
            // If we found any foreign key fields, populate the relation
            if !relation.foreign_key_fields.is_empty() {
                // Set is_composite only if we have multiple fields
                if relation.foreign_key_fields.len() > 1 {
                    relation.is_composite = true;
                }
                
                // Populate the old single-field approach for backward compatibility
                relation.foreign_key_field = Some(relation.foreign_key_fields[0].clone());
                relation.primary_key_field = Some(relation.target_primary_key_fields[0].clone());
                
                // Populate foreign key types and columns
                for field_name in &relation.foreign_key_fields {
                    // field_name is already snake_case
                    if let Some(field) = model_fields.iter().find(|f| {
                        f.ident.as_ref().unwrap().to_string() == *field_name
                    }) {
                        relation.foreign_key_types.push(field.ty.clone());
                    }
                }
                
                for field_name in &relation.foreign_key_fields {
                    relation.foreign_key_columns.push(field_name.clone());
                }
                
                for field_name in &relation.target_primary_key_fields {
                    relation.target_primary_key_columns.push(field_name.clone());
                }
                
                // Extract target entity name
                if let Some(segment) = relation.target.segments.last() {
                    let entity_name = segment.ident.to_string();
                    relation.target_entity_name = Some(entity_name);
                }
                
                // Create target unique param
                if !relation.foreign_key_fields.is_empty() {
                    let mut unique_param_path = relation.target.clone();
                    unique_param_path.segments.push(syn::PathSegment {
                        ident: syn::Ident::new("UniqueWhereParam", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None,
                    });
                    relation.target_unique_param = Some(unique_param_path);
                }
                
                relations.push(relation);
            }
        }
    }
    
    relations
}

fn parse_target_path(target_str: &str) -> syn::Path {
    let target_path = syn::parse_str::<syn::Path>(target_str)
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
    
    new_path
}

fn extract_field_name(column_str: &str) -> String {
    // Extract field name from column reference like "Column::StudentId" -> "StudentId"
    // This returns the Column enum variant name (PascalCase)
    if let Some(colon_pos) = column_str.rfind("::") {
        column_str[colon_pos + 2..].to_string()
    } else {
        column_str.to_string()
    }
}