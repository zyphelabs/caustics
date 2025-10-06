use super::{extract_relations, generate_relation_submodules, RelationKind};
use crate::common::is_option;
use crate::name_resolution::EntityNameContext;
use crate::primary_key::{
    extract_primary_key_info, get_primary_key_column_name, get_primary_key_field_ident,
    get_primary_key_field_name,
};
use crate::validation::{validate_foreign_key_column, validate_table_name};
use crate::where_param::generate_where_param_logic;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

#[allow(clippy::cmp_owned)]
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::possible_missing_else)]
#[allow(clippy::unnecessary_filter_map)]
#[allow(clippy::useless_conversion)]
#[allow(clippy::if_same_then_else)]
pub fn generate_entity(
    model_ast: DeriveInput,
    relation_ast: DeriveInput,
    namespace: String,
    full_mod_path: &syn::Path,
) -> Result<TokenStream, proc_macro2::TokenStream> {
    // Extract fields
    let fields = match &model_ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named.named.iter().collect::<Vec<_>>(),
            _ => {
                return Err(
                    quote! { compile_error!("#[caustics] requires a named-field struct for the Model"); },
                );
            }
        },
        _ => {
            return Err(quote! { compile_error!("#[caustics] must be applied to a struct"); });
        }
    };

    // Extract current entity's table name
    let current_table_name = validate_table_name(&model_ast)?;

    // Create centralized entity name context using extracted metadata
    // This avoids fragile string manipulation by using the actual extracted information
    // The entity name should be derived from the module name, not the struct name
    let module_name = full_mod_path
        .segments
        .last()
        .expect("Invalid module path - this should not happen in valid code")
        .ident
        .to_string();
    let entity_name = module_name.to_pascal_case();
    let entity_context = EntityNameContext::from_metadata(&entity_name, &current_table_name);

    // Extract relations from relation_ast
    let relations = extract_relations(&relation_ast, &fields, &current_table_name);

    // Extract primary key field name from current entity
    let current_primary_key = get_primary_key_field_name(&fields);

    // Generate per-relation fetcher arms
    let mut relation_names = Vec::new();
    let mut relation_fetcher_bodies = Vec::new();
    for rel in &relations {
        let rel_name_snake = rel.name.to_snake_case();
        relation_names.push(quote! { #rel_name_snake });
        let target = &rel.target;
        let foreign_key_column = validate_foreign_key_column(
            &rel.name,
            &rel.foreign_key_column,
            proc_macro2::Span::call_site(),
        )?;
        let foreign_key_column_ident = format_ident!("{}", foreign_key_column.to_pascal_case());
        let foreign_key_column_snake = foreign_key_column.to_snake_case();
        let relation_name_str = rel.name.to_snake_case();

        // Extract primary key field from the relation definition
        // For belongs_to relations: the 'to' column is the primary key of the target entity
        // For has_many relations: we need to resolve from the current entity's primary key
        let target_primary_key = if let Some(pk_field) = &rel.primary_key_field {
            // If explicitly specified in relation, use it
            pk_field.clone()
        } else {
            // Extract from the relation based on relation type
            match rel.kind {
                RelationKind::BelongsTo => {
                    // For belongs_to, the 'to' column is the primary key of the target entity
                    // Example: to = "super::user::Column::Id" -> primary key is "id"
                    foreign_key_column.clone()
                }
                RelationKind::HasMany => {
                    // For has_many, we need to use the current entity's primary key
                    current_primary_key.clone()
                }
            }
        };
        let target_primary_key_lit =
            syn::LitStr::new(&target_primary_key, proc_macro2::Span::call_site());
        let target_primary_key_str = target_primary_key.clone();

        // Extract target entity name from the relation
        let target_entity_name = if let Some(entity_name) = &rel.target_entity_name {
            entity_name.clone()
        } else {
            // Fallback: extract from target path
            rel.target
                .segments
                .last()
                .expect("Failed to parse relation - this should not happen in valid code")
                .ident
                .to_string()
                .to_lowercase()
        };

        let fetcher_body = if matches!(rel.kind, RelationKind::HasMany) {
            quote! {
            let mut query = #target::Entity::find();
            if let Some(fk_value) = foreign_key_value {
                if let Ok(value) = crate::__caustics_convert_key_for_sea_orm(#target_entity_name, #foreign_key_column_snake, fk_value) {
                    // Use raw SQL expression to bypass SeaORM's typed API
                    query = query.filter(sea_query::Expr::cust_with_values(
                        &format!("{} = ?", sea_orm::Iden::to_string(&#target::Column::#foreign_key_column_ident)),
                        [value]
                    ));
                }
            }

            // Apply child-level filters from RelationFilter
            if !filter.filters.is_empty() {
                let mut cond = Condition::all();
                for f in &filter.filters {
                    if let Some(col) = #target::column_from_str(&f.field) {
                        use sea_orm::IntoSimpleExpr;
                        let col_expr = col.into_simple_expr();
                        match &f.operation {
                            caustics::FieldOp::Equals(v) => {
                                let val = caustics::parse_string_to_sea_orm_value(v);
                                cond = cond.add(Expr::expr(col_expr.clone()).eq(val));
                            }
                            caustics::FieldOp::NotEquals(v) => {
                                let val = caustics::parse_string_to_sea_orm_value(v);
                                cond = cond.add(Expr::expr(col_expr.clone()).ne(val));
                            }
                            caustics::FieldOp::Contains(s) => {
                                let pat = format!("%{}%", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::StartsWith(s) => {
                                let pat = format!("{}%", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::EndsWith(s) => {
                                let pat = format!("%{}", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::IsNull => {
                                cond = cond.add(Expr::expr(col_expr.clone()).is_null());
                            }
                            caustics::FieldOp::IsNotNull => {
                                cond = cond.add(Expr::expr(col_expr.clone()).is_not_null());
                            }
                            _ => {}
                        }
                    }
                }
                query = query.filter(cond);
            }

            // Apply cursor (primary key-based cursor)
            if let Some(ref cur) = filter.cursor_id {
                query = query.filter(#target::Column::Id.gt(cur.to_db_value()));
            }

            // Apply order_by on any recognized column
            for (field, dir) in &filter.order_by {
                if let Some(col) = #target::column_from_str(field) {
                    let ord = match dir { caustics::SortOrder::Asc => sea_orm::Order::Asc, caustics::SortOrder::Desc => sea_orm::Order::Desc };
                    query = query.order_by(col, ord);
                }
            }

            if let Some(offset) = filter.skip { if offset > 0 { query = query.offset(offset as u64); } }
            if let Some(limit) = filter.take { if limit >= 0 { query = query.limit(limit as u64); } }

            let mut q_exec = query;
            // Apply distinct if requested
            if filter.distinct {
                q_exec = q_exec.distinct();
            }

            // Check if field selection is being used
            let has_field_selection = filter.nested_select_aliases.as_ref()
                .map(|aliases| !aliases.is_empty())
                .unwrap_or(false);

            if has_field_selection {
                // For field selection, compute required fields and fetch only those from database
                let selected_fields = filter.nested_select_aliases.as_ref()
                    .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                // Compute required fields: selected fields + defensive fields (primary key, foreign keys, unique fields)
                let required_fields = {
                    let mut fields = std::collections::HashSet::<&'static str>::new();
                    // Add selected fields
                    for field in &selected_fields {
                        fields.insert(field);
                    }

                    // Get target entity metadata to include defensive fields
                    let target_entity_name = stringify!(#target);
                    // Convert module name to entity name (snake_case to PascalCase)
                    let entity_name = target_entity_name.split('_')
                        .map(|s| {
                            let mut chars = s.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().chain(chars).collect(),
                            }
                        })
                        .collect::<String>();
                    // Always include primary key for relation traversal
                    fields.insert(#current_primary_key);
                    fields
                };

                // Convert required fields to SeaORM expressions (like main queries do)
                let mut selected_fields_exprs = Vec::new();
                for field in &required_fields {
                    if let Some(expr) = #target::Selected::column_for_alias(field) {
                        selected_fields_exprs.push((expr, ToString::to_string(&field)));
                    }
                }

                // Apply database-level field selection using raw SQL approach (like main queries)
                let vec_selected = if selected_fields_exprs.is_empty() {
                    // Fetch all fields if no valid expressions found
                    let models = q_exec.all(conn).await?;
                    models.into_iter()
                        .map(|model| #target::Selected::from_model(model, &[]))
                        .collect::<Vec<_>>()
                } else {
                    // Use raw SQL approach with select_only() + expr_as() (like main queries)
                    let mut select_query = q_exec.select_only();
                    for (expr, alias) in &selected_fields_exprs {
                        select_query = select_query.expr_as(expr.clone(), alias.as_str());
                    }

                    // Build and execute raw SQL query (like main queries do)
                    use sea_orm::QueryTrait;
                    let stmt = select_query.build(conn.get_database_backend());
                    let rows = conn.query_all(stmt).await?;

                    // Use fill_from_row method (like main queries do)
                    use caustics::EntitySelection;
                    rows.into_iter()
                        .map(|row| {
                            let field_names: Vec<&str> = required_fields.iter().map(|s| s.as_ref()).collect();
                            #target::Selected::fill_from_row(&row, &field_names)
                        })
                        .collect::<Vec<_>>()
                };

                Ok(Box::new(Some(vec_selected)) as Box<dyn std::any::Any + Send>)
            } else {
                // No field selection - return ModelWithRelations objects with all fields
            let vec_with_rel = q_exec.all(conn).await?
                        .into_iter()
                .map(|model| #target::ModelWithRelations::from_model(model))
                .collect::<Vec<_>>();

            Ok(Box::new(Some(vec_with_rel)) as Box<dyn std::any::Any + Send>)
            }
                }
        } else {
            // belongs_to relation - query the TARGET entity by its primary key, using the current entity's foreign key value
            let is_nullable_fk = rel.is_nullable;
            let target_entity = &rel.target;
            let target_entity_type = quote! { #target_entity::Entity };
            let target_model_with_rel = quote! { #target_entity::ModelWithRelations };
            let target_unique_param = quote! { #target_entity::UniqueWhereParam };

            // Get the primary key field name from the relation definition or use dynamic detection
            let primary_key_field_name = target_primary_key_str.to_snake_case();
            let primary_key_pascal = target_primary_key_str
                .chars()
                .next()
                .expect("Failed to parse relation - this should not happen in valid code")
                .to_uppercase()
                .collect::<String>()
                + &target_primary_key_str[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);

            // Extract primary key field from the relation definition
            // For belongs_to relations: the 'to' column is the primary key of the target entity
            // For has_many relations: we need to resolve from the current entity's primary key
            let target_primary_key = if let Some(pk_field) = &rel.primary_key_field {
                // If explicitly specified in relation, use it
                pk_field.clone()
            } else {
                // Extract from the relation based on relation type
                match rel.kind {
                    RelationKind::BelongsTo => {
                        // For belongs_to, the 'to' column is the primary key of the target entity
                        // Example: to = "super::user::Column::Id" -> primary key is "id"
                        rel.foreign_key_column.clone()
                            .unwrap_or_else(|| {
                                panic!("No primary key field could be determined for relation '{}'. Please specify 'to' attribute with target column.", rel.name)
                            })
                    }
                    RelationKind::HasMany => {
                        // For has_many, we need to use the current entity's primary key
                        current_primary_key.clone()
                    }
                }
            };
            // Extract foreign key information from the current relation
            let target_foreign_keys = if let Some(fk_field) = &rel.foreign_key_field {
                vec![fk_field.clone()]
            } else {
                Vec::new()
            };

            if is_nullable_fk {
                quote! {
                    if let Some(fk_value) = foreign_key_value {
                let condition = #target_unique_param::#primary_key_variant(fk_value);
                            let mut query = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into());

                            // Check if field selection is being used
                            let has_field_selection = filter.nested_select_aliases.as_ref()
                                .map(|aliases| !aliases.is_empty())
                                .unwrap_or(false);

                            if has_field_selection {
                                // For field selection, compute required fields and fetch only those from database
                                let selected_fields = filter.nested_select_aliases.as_ref()
                                    .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                                    .unwrap_or_default();

                                // Compute required fields: selected fields + defensive fields (primary key, foreign keys, unique fields)
                                let required_fields = {
                                    let mut fields = std::collections::HashSet::<&'static str>::new();
                                    // Add selected fields
                                    for field in &selected_fields {
                                        fields.insert(field);
                                    }

                                    // Add target entity's foreign key fields for nested relation traversal
                                    for fk_field in [#(#target_foreign_keys),*] {
                                        fields.insert(fk_field);
                                    }

                                    // Always include primary key for relation traversal
                                    fields.insert(#target_primary_key);
                                    fields
                                };

                                // Apply database-level field selection using raw SQL approach (like main queries)
                                let selected_fields_exprs: Vec<_> = required_fields.iter()
                                    .filter_map(|field| {
                                        if let Some(col) = #target_entity::column_from_str(field) {
                                            Some((col.into_simple_expr(), ToString::to_string(&field)))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>();

                                // Apply database-level field selection using raw SQL approach (like main queries)
                                let opt_selected = if selected_fields_exprs.is_empty() {
                                    // Fetch all fields if no valid expressions found
                                    let models = query.all(conn).await?;
                                    let selected_vec: Vec<#target_entity::Selected> = models.into_iter().map(|m| #target_entity::Selected::from_model(m, &[])).collect();
                                    Some(selected_vec)
                                } else {
                                    // Use raw SQL approach with select_only() + expr_as() (like main queries)
                                    let mut select_query = query.select_only();
                                    for (expr, alias) in &selected_fields_exprs {
                                        select_query = select_query.expr_as(expr.clone(), alias.as_str());
                                    }

                                    // Build and execute raw SQL query (like main queries do)
                                    use sea_orm::QueryTrait;
                                    let stmt = select_query.build(conn.get_database_backend());
                                    let rows = conn.query_all(stmt).await?;

                                    // Use fill_from_row method (like main queries do)
                                    use caustics::EntitySelection;
                                    let selected_vec: Vec<#target_entity::Selected> = rows.into_iter().map(|row| {
                                        let field_names: Vec<&str> = required_fields.iter().map(|s| s.as_ref()).collect();
                                        #target_entity::Selected::fill_from_row(&row, &field_names)
                                    }).collect();
                                    Some(selected_vec)
                                };

                                // Return Selected object directly (no conversion needed)
                                return Ok(Box::new(opt_selected) as Box<dyn std::any::Any + Send>);
                            } else {
                                // No field selection - return Selected objects with all fields
                                let models = query.all(conn).await?;
                                let selected_vec: Vec<#target_entity::Selected> = models.into_iter().map(|model| #target_entity::Selected::from_model(model, &[])).collect();
                                let with_rel = Some(selected_vec);
                                return Ok(Box::new(with_rel) as Box<dyn std::any::Any + Send>);
                            }
                        } else {
                            return Ok(Box::new(None::<#target_entity::ModelWithRelations>) as Box<dyn std::any::Any + Send>);
                    }
                }
            } else {
                quote! {
                if let Some(fk_value) = foreign_key_value {
                        let condition = #target_unique_param::#primary_key_variant(fk_value);
                        let mut query = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into());

                        // Check if field selection is being used
                        let has_field_selection = filter.nested_select_aliases.as_ref()
                            .map(|aliases| !aliases.is_empty())
                            .unwrap_or(false);


                        if has_field_selection {
                            // For field selection, compute required fields and fetch only those from database
                            let selected_fields = filter.nested_select_aliases.as_ref()
                                .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                                .unwrap_or_default();

                            // Compute required fields: selected fields + defensive fields (primary key, foreign keys, unique fields)
                            // For BelongsTo relations, always include the foreign key field
                            let required_fields = {
                                let mut fields = std::collections::HashSet::<&str>::new();
                                // Add selected fields
                                for field in &selected_fields {
                                    fields.insert(field);
                                }

                                // Always include primary key for relation traversal
                                fields.insert(#target_primary_key);

                                // For BelongsTo relations, always include the foreign key field
                                fields.insert(#foreign_key_column);

                                // Add foreign key fields for nested relation traversal using metadata
                                for fk_field in [#(#target_foreign_keys),*] {
                                    fields.insert(fk_field);
                                }

                                fields
                            };

                            // Convert required fields to SeaORM expressions (like main queries do)
                            let mut selected_fields_exprs = Vec::new();
                            for field in &required_fields {
                                if let Some(expr) = #target_entity::Selected::column_for_alias(field) {
                                    selected_fields_exprs.push((expr, ToString::to_string(&field)));
                                }
                            }

                            // Apply database-level field selection using raw SQL approach (like main queries)
                            let opt_selected = if selected_fields_exprs.is_empty() {
                                // Fetch all fields if no valid expressions found
                                let model = query.one(conn).await?;
                                model.map(|m| #target_entity::Selected::from_model(m, &[]))
                            } else {
                                // Use raw SQL approach with select_only() + expr_as() (like main queries)
                                let mut select_query = query.select_only();
                                for (expr, alias) in &selected_fields_exprs {
                                    select_query = select_query.expr_as(expr.clone(), alias.as_str());
                                }

                                // Build and execute raw SQL query (like main queries do)
                                use sea_orm::QueryTrait;
                                let stmt = select_query.build(conn.get_database_backend());
                                let row_opt = conn.query_one(stmt).await?;

                                // Use fill_from_row method (like main queries do)
                                use caustics::EntitySelection;
                                row_opt.map(|row| {
                                    let field_names: Vec<&str> = selected_fields.iter().map(|s| s.as_ref()).collect();
                                    #target_entity::Selected::fill_from_row(&row, &field_names)
                                })
                            };

                            // Return Selected object directly (no conversion needed)
                            return Ok(Box::new(opt_selected) as Box<dyn std::any::Any + Send>);
                        } else {
                            // No field selection - return Selected objects with all fields
                            let opt_model = query.one(conn).await?;
                            let with_rel = opt_model.map(|model| #target_entity::Selected::from_model(model, &[]));
                            return Ok(Box::new(with_rel) as Box<dyn std::any::Any + Send>);
                        }
                } else {
                    Ok(Box::new(None::<#target_entity::ModelWithRelations>) as Box<dyn std::any::Any + Send>)
                    }
                }
            }
        };
        relation_fetcher_bodies.push(fetcher_body);
    }

    // Generate per-relation fetcher arms for Selected types (copy from ModelWithRelations version but return Selected)
    let mut relation_fetcher_bodies_selected = Vec::new();
    for rel in &relations {
        let rel_name_snake = rel.name.to_snake_case();
        let target = &rel.target;
        let foreign_key_column = validate_foreign_key_column(
            &rel.name,
            &rel.foreign_key_column,
            proc_macro2::Span::call_site(),
        )?;
        let foreign_key_column_ident = format_ident!("{}", foreign_key_column.to_pascal_case());
        let foreign_key_column_str = foreign_key_column.to_snake_case();
        let relation_name_str = rel.name.to_snake_case();

        // Extract primary key field from the relation definition
        // For belongs_to relations: the 'to' column is the primary key of the target entity
        // For has_many relations: we need to resolve from the current entity's primary key
        let target_primary_key = if let Some(pk_field) = &rel.primary_key_field {
            // If explicitly specified in relation, use it
            pk_field.clone()
        } else {
            // Extract from the relation based on relation type
            match rel.kind {
                RelationKind::BelongsTo => {
                    // For belongs_to, the 'to' column is the primary key of the target entity
                    // Example: to = "super::user::Column::Id" -> primary key is "id"
                    foreign_key_column.clone()
                }
                RelationKind::HasMany => {
                    // For has_many, we need to use the current entity's primary key
                    current_primary_key.clone()
                }
            }
        };
        let target_primary_key_str = target_primary_key.clone();

        // Extract target entity name from the relation
        let target_entity_name = if let Some(entity_name) = &rel.target_entity_name {
            entity_name.clone()
        } else {
            // Fallback: extract from target path
            rel.target
                .segments
                .last()
                .expect("Failed to parse relation - this should not happen in valid code")
                .ident
                .to_string()
                .to_lowercase()
        };

        // Copy the exact same logic as ModelWithRelations version but change the final mapping to Selected
        let fetcher_body = if matches!(rel.kind, RelationKind::HasMany) {
            // Get the primary key field name from the relation definition or use dynamic detection
            let primary_key_field_name = target_primary_key_str.to_snake_case();

            quote! {
            let mut query = #target::Entity::find();
            if let Some(fk_value) = foreign_key_value {
                if let Ok(value) = crate::__caustics_convert_key_for_sea_orm(#target::Entity::default().table_name(), #primary_key_field_name, fk_value) {
                    // Use raw SQL expression to bypass SeaORM's typed API
                    query = query.filter(sea_query::Expr::cust_with_values(
                        &format!("{} = ?", sea_orm::Iden::to_string(&#target::Column::#foreign_key_column_ident)),
                        [value]
                    ));
                }
            }
            use sea_orm::QueryTrait;
            let query_sql = query.build(conn.get_database_backend());

            // Check if field selection is being used
            let has_field_selection = filter.nested_select_aliases.as_ref()
                .map(|aliases| !aliases.is_empty())
                .unwrap_or(false);

            // Apply child-level filters from RelationFilter
            if !filter.filters.is_empty() {
                let mut cond = Condition::all();
                for f in &filter.filters {
                    if let Some(col) = #target::column_from_str(&f.field) {
                        use sea_orm::IntoSimpleExpr;
                        let col_expr = col.into_simple_expr();
                        match &f.operation {
                            caustics::FieldOp::Equals(v) => {
                                let val = caustics::parse_string_to_sea_orm_value(v);
                                cond = cond.add(Expr::expr(col_expr.clone()).eq(val));
                            }
                            caustics::FieldOp::NotEquals(v) => {
                                let val = caustics::parse_string_to_sea_orm_value(v);
                                cond = cond.add(Expr::expr(col_expr.clone()).ne(val));
                            }
                            caustics::FieldOp::Contains(s) => {
                                let pat = format!("%{}%", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::StartsWith(s) => {
                                let pat = format!("{}%", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::EndsWith(s) => {
                                let pat = format!("%{}", s);
                                cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                            }
                            caustics::FieldOp::IsNull => {
                                cond = cond.add(Expr::expr(col_expr.clone()).is_null());
                            }
                            caustics::FieldOp::IsNotNull => {
                                cond = cond.add(Expr::expr(col_expr.clone()).is_not_null());
                            }
                            _ => {}
                        }
                    }
                }
                query = query.filter(cond);
                            }

                            // Apply cursor (primary key-based cursor)
                            if let Some(ref cur) = filter.cursor_id {
                                query = query.filter(#target::Column::Id.gt(cur.to_db_value()));
                            }

                            // Apply ordering
                            for (field, order) in &filter.order_by {
                if let Some(col) = #target::column_from_str(field) {
                    use sea_orm::IntoSimpleExpr;
                    let ord = match order {
                        caustics::SortOrder::Asc => sea_orm::Order::Asc,
                        caustics::SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    query = query.order_by(col.into_simple_expr(), ord);
                                }
                            }

                            // Apply pagination
                            if let Some(take) = filter.take {
                query = query.limit(take as u64);
                            }
                            if let Some(skip) = filter.skip {
                query = query.offset(skip as u64);
            }

            let mut q_exec = query;
            // Apply distinct if requested
            if filter.distinct {
                q_exec = q_exec.distinct();
            }

            if has_field_selection {
                // For field selection, compute required fields and fetch only those from database
                let selected_fields = filter.nested_select_aliases.as_ref()
                    .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                                // Compute required fields: selected fields + defensive fields (computed at build time)
                                let required_fields = {
                                    let mut fields = std::collections::HashSet::<&str>::new();
                                    // Add selected fields
                                    fields.extend(selected_fields.iter().map(|s| *s));
                                    // Add defensive fields for relation traversal
                                    // Get target entity metadata to find primary key field
                                    let target_entity_name = stringify!(#target);
                                    let entity_name = target_entity_name.split('_').map(|s| {
                                        let mut chars = s.chars();
                                        match chars.next() {
                                            None => String::new(),
                                            Some(first) => first.to_uppercase().chain(chars).collect(),
                                        }
                                    }).collect::<String>();
                                    // Always include primary key for relation traversal
                                    fields.insert(#current_primary_key);
                                    // Add foreign key field for this relation
                                    fields.insert(#foreign_key_column_str);
                    // Add all foreign key fields for nested relation traversal
                    // (Metadata system handles this dynamically)

                    // Add foreign key fields for nested relation traversal
                    // The target entity's relation metadata is not available when generating source entity's relation fetchers

                    // Fallback to fetching all fields when target entity metadata is not available
                    fields
                };

                // Convert required fields to SeaORM expressions (like main queries do)
                let mut selected_fields_exprs = Vec::new();
                for field in &required_fields {
                    if let Some(expr) = #target::Selected::column_for_alias(field) {
                        selected_fields_exprs.push((expr, ToString::to_string(&field)));
                    }
                }

                // Apply database-level field selection using raw SQL approach (like main queries)
                let selected_models = if selected_fields_exprs.is_empty() {
                    // Fetch all fields if no valid expressions found
                let models = q_exec.all(conn).await?;
                    models.into_iter().map(|m| #target::Selected::from_model(m, &[])).collect()
                } else {
                    // Use raw SQL approach with select_only() + expr_as() (like main queries)
                    let mut select_query = q_exec.select_only();
                    for (expr, alias) in &selected_fields_exprs {
                        select_query = select_query.expr_as(expr.clone(), alias.as_str());
                    }

                    // Build and execute raw SQL query (like main queries do)
                    use sea_orm::QueryTrait;
                    let stmt = select_query.build(conn.get_database_backend());
                    let rows = conn.query_all(stmt).await?;

                    // Use fill_from_row method (like main queries do)
                    use caustics::EntitySelection;
                    let field_names: Vec<&str> = required_fields.iter().map(|s| s.as_ref()).collect();
                    rows.into_iter().map(|row| {
                        #target::Selected::fill_from_row(&row, &field_names)
                    }).collect::<Vec<#target::Selected>>()
                };

                // Return Selected objects directly (no conversion needed)
                let vec_with_rel = selected_models;
                Ok(Box::new(Some(vec_with_rel)) as Box<dyn std::any::Any + Send>)
            } else {
                // No field selection - return Selected objects with all fields
                let models = q_exec.all(conn).await?;
                let vec_with_rel = models
                            .into_iter()
                    .map(|model| #target::Selected::from_model(model, &[]))
                    .collect::<Vec<_>>();
                Ok(Box::new(Some(vec_with_rel)) as Box<dyn std::any::Any + Send>)
            }
                    }
        } else {
            // belongs_to relation - query the TARGET entity by its primary key, using the current entity's foreign key value
            let target_entity_type = quote! { #target::Entity };
            let target_unique_param = quote! { #target::UniqueWhereParam };
            let primary_key_variant = format_ident!("IdEquals");

            let is_nullable_fk = rel.foreign_key_field.as_ref().is_some_and(|fk_field_name| {
                fields
                    .iter()
                    .find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
                    })
                    .is_some_and(|field| is_option(&field.ty))
            });

            if is_nullable_fk {
                // Metadata system handles defensive fields dynamically

                quote! {
                    if let Some(fk_value) = foreign_key_value {
                            let condition = #target_unique_param::#primary_key_variant(fk_value);
                            let mut query = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into());

                            // Check if field selection is being used
                            let has_field_selection = filter.nested_select_aliases.as_ref()
                                .map(|aliases| !aliases.is_empty())
                                .unwrap_or(false);

                            if has_field_selection {
                                // For field selection, compute required fields and fetch only those from database
                                let selected_fields = filter.nested_select_aliases.as_ref()
                                    .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                                    .unwrap_or_default();

                                // Compute required fields: selected fields + defensive fields (computed at build time)
                                let required_fields = {
                                    let mut fields = std::collections::HashSet::<&str>::new();
                                    // Add selected fields
                                    fields.extend(selected_fields.iter().map(|s| *s));
                                    // Add defensive fields for relation traversal
                                    // Get target entity metadata to find primary key field
                                    let target_entity_name = stringify!(#target);
                                    // Clean up the entity name by removing namespace and extra spaces
                                    let entity_name = target_entity_name
                                        .split("::")
                                        .last()
                                        .unwrap_or(target_entity_name)
                                        .trim()
                                        .split('_')
                                        .map(|s| {
                                            let mut chars = s.chars();
                                            match chars.next() {
                                                None => String::new(),
                                                Some(first) => first.to_uppercase().chain(chars).collect(),
                                            }
                                        })
                                        .collect::<String>();
                                    // Add primary key field dynamically
                                    fields.insert(#target_primary_key);
                                    // Add foreign key field for this relation
                                    fields.insert(#foreign_key_column_str);
                                    // Add all foreign key fields for nested relation traversal
                                    // (Metadata system handles this dynamically)

                                    // Add all foreign key fields for nested relation traversal
                                    // Use the generated client's registry to get foreign key fields
                                    if let Some(metadata) = crate::get_entity_metadata(&entity_name) {
                                        for fk_field in metadata.foreign_key_fields {
                                            fields.insert(Box::leak(std::string::ToString::to_string(fk_field).into_boxed_str()));
                                        }
                                    } else {
                                        panic!("No metadata found for entity '{}'", entity_name);
                                    }

                                    fields
                                };

                                // Convert required fields to SeaORM expressions (like main queries do)
                                let mut selected_fields_exprs = Vec::new();
                                for field in &required_fields {
                                    if let Some(expr) = #target::Selected::column_for_alias(field) {
                                        selected_fields_exprs.push((expr, ToString::to_string(&field)));
                                    }
                                }


                                // Apply database-level field selection using raw SQL approach (like main queries)
                                let opt_selected = if selected_fields_exprs.is_empty() {
                                    // Fetch all fields if no valid expressions found
                                    let model = query.one(conn).await?;
                                    model.map(|m| #target::Selected::from_model(m, &[]))
                                } else {
                                    // Use raw SQL approach with select_only() + expr_as() (like main queries)
                                    let mut select_query = query.select_only();
                                    for (expr, alias) in &selected_fields_exprs {
                                        select_query = select_query.expr_as(expr.clone(), alias.as_str());
                                    }

                                    // Build and execute raw SQL query (like main queries do)
                                    use sea_orm::QueryTrait;
                                    let stmt = select_query.build(conn.get_database_backend());
                                    let row_opt = conn.query_one(stmt).await?;

                                // Use fill_from_row method (like main queries do)
                                use caustics::EntitySelection;
                                row_opt.map(|row| {
                                    let field_names: Vec<&str> = required_fields.iter().map(|s| s.as_ref()).collect();
                                    #target::Selected::fill_from_row(&row, &field_names)
                                })
                                };

                                // Return Selected object directly (no conversion needed)
                                return Ok(Box::new(opt_selected) as Box<dyn std::any::Any + Send>);
                            } else {
                                // No field selection - return Selected objects with all fields
                                let opt_model = query.one(conn).await?;
                                let with_rel = opt_model.map(|model| #target::Selected::from_model(model, &[]));
                                return Ok(Box::new(with_rel) as Box<dyn std::any::Any + Send>);
                            }
                        } else {
                            return Ok(Box::new(None::<#target::Selected>) as Box<dyn std::any::Any + Send>);
                    }
                }
            } else {
                // Metadata system handles defensive fields dynamically

                quote! {
                        if let Some(fk_value) = foreign_key_value {
                            let condition = #target_unique_param::#primary_key_variant(fk_value);
                            let mut query = <#target_entity_type as EntityTrait>::find().filter::<sea_query::Condition>(condition.into());

                // Apply database-level field selection optimization
                // For relation fetchers, we need all fields to properly construct the target entity
                // The actual field selection optimization happens at the Selected struct level

                            // Check if field selection is being used
                            let has_field_selection = filter.nested_select_aliases.as_ref()
                                .map(|aliases| !aliases.is_empty())
                                .unwrap_or(false);

                            if has_field_selection {
                                    // For field selection, compute required fields and fetch only those from database
                                let selected_fields = filter.nested_select_aliases.as_ref()
                                    .map(|aliases| aliases.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                                    .unwrap_or_default();

                                    // Compute required fields: selected fields + defensive fields (computed at build time)
                                    let required_fields = {
                                        let mut fields = std::collections::HashSet::<&str>::new();
                                        // Add selected fields
                                        fields.extend(selected_fields.iter().map(|s| *s));
                                        // Add defensive fields for relation traversal
                                        // Get target entity metadata to find primary key field
                                        let target_entity_name = stringify!(#target);
                                        // Clean up the entity name by removing namespace and extra spaces
                                        let entity_name = target_entity_name
                                            .split("::")
                                            .last()
                                            .unwrap_or(target_entity_name)
                                            .trim()
                                            .split('_')
                                            .map(|s| {
                                                let mut chars = s.chars();
                                                match chars.next() {
                                                    None => String::new(),
                                                    Some(first) => first.to_uppercase().chain(chars).collect(),
                                                }
                                            })
                                            .collect::<String>();
                                        // Add primary key field dynamically
                                        fields.insert(#target_primary_key);
                                        // Add foreign key field for this relation
                                        fields.insert(#foreign_key_column_str);
                                        // Add all foreign key fields for nested relation traversal
                                        // (Metadata system handles this dynamically)

                                        // Add all foreign key fields for nested relation traversal
                                        // Use the generated client's registry to get foreign key fields
                                        if let Some(metadata) = crate::get_entity_metadata(&entity_name) {
                                            for fk_field in metadata.foreign_key_fields {
                                                fields.insert(Box::leak(std::string::ToString::to_string(fk_field).into_boxed_str()));
                                            }
                                        } else {
                                            // Fallback: include all "_id" fields if registry is not available
                                            use sea_orm::Iterable;
                                            let target_columns = #target::Column::iter();
                                            for column in target_columns {
                                                let column_name = format!("{:?}", column).to_lowercase();
                                                if column_name.ends_with("_id") {
                                                    fields.insert(Box::leak(column_name.into_boxed_str()));
                                                }
                                            }
                                        }

                                       fields
                                    };

                                    // Convert required fields to SeaORM expressions (like main queries do)
                                    let mut selected_fields_exprs = Vec::new();
                                    for field in &required_fields {
                                        if let Some(expr) = #target::Selected::column_for_alias(field) {
                                            selected_fields_exprs.push((expr, ToString::to_string(&field)));
                                        }
                                    }

                                    // Apply database-level field selection using the same approach as main queries
                                    // Apply database-level field selection using raw SQL approach (like main queries)
                                    let opt_selected = if selected_fields_exprs.is_empty() {
                                        // Fetch all fields if no valid expressions found
                                        let model = query.one(conn).await?;
                                        model.map(|m| #target::Selected::from_model(m, &[]))
                                    } else {
                                        // Use raw SQL approach with select_only() + expr_as() (like main queries)
                                        let mut select_query = query.select_only();
                                        for (expr, alias) in &selected_fields_exprs {
                                            select_query = select_query.expr_as(expr.clone(), alias.as_str());
                                        }

                                        // Build and execute raw SQL query (like main queries do)
                                        use sea_orm::QueryTrait;
                                        let stmt = select_query.build(conn.get_database_backend());
                                        let row_opt = conn.query_one(stmt).await?;

                                    // Use fill_from_row method (like main queries do)
                                    use caustics::EntitySelection;
                                    row_opt.map(|row| {
                                        let field_names: Vec<&str> = required_fields.iter().map(|s| s.as_ref()).collect();
                                        #target::Selected::fill_from_row(&row, &field_names)
                                    })
                                    };

                                    // Return Selected object directly (no conversion needed)
                                    return Ok(Box::new(opt_selected) as Box<dyn std::any::Any + Send>);
                            } else {
                                // No field selection - return Selected objects with all fields
                                let opt_model = query.one(conn).await?;
                                let with_rel = opt_model.map(|model| #target::Selected::from_model(model, &[]));
                                return Ok(Box::new(with_rel) as Box<dyn std::any::Any + Send>);
                            }
                            } else {
                        Ok(Box::new(None::<#target::Selected>) as Box<dyn std::any::Any + Send>)
                        }
                    }
            }
        };
        relation_fetcher_bodies_selected.push(fetcher_body);
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

    // Get primary key information using centralized utilities
    let primary_key_info = extract_primary_key_info(&fields);
    let current_primary_key_ident = get_primary_key_field_ident(&fields);
    let current_primary_key_field_name = get_primary_key_field_name(&fields);
    let current_primary_key_column_name = get_primary_key_column_name(&fields);

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
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
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
            let name = field.ident.as_ref().expect("Field has no identifier");
            quote! { pub #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate function arguments for required fields (no pub)
    let required_fn_args = required_fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let name = field.ident.as_ref().expect("Field has no identifier");
            quote! { #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate initializers for required fields (no pub)
    let required_inits = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate assignments for required fields (self.#name)
    let required_assigns = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            quote! { model.#name = sea_orm::ActiveValue::Set(self.#name); }
        })
        .collect::<Vec<_>>();

    // Check if primary key is UUID type and generate UUID generation code
    let uuid_pk_check = if let Some(pk_field) = primary_key_fields.first() {
        if let syn::Type::Path(type_path) = &pk_field.ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Uuid" {
                    quote! {
                        if model.#current_primary_key_ident == sea_orm::ActiveValue::NotSet {
                            model.#current_primary_key_ident = sea_orm::ActiveValue::Set(uuid::Uuid::new_v4());
                        }
                    }
                } else {
                    quote! {}
                }
            } else {
                quote! {}
            }
        } else {
            quote! {}
        }
    } else {
        quote! {}
    };

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
                    let fk_field_name = relation
                        .foreign_key_field
                        .as_ref()
                        .expect("Foreign key field not specified");
                    if let Some(field) = fields.iter().find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Check if the foreign key field is not nullable (not Option<T>)
                    // Only required relations should be function arguments
                    let fk_field_name = relation
                        .foreign_key_field
                        .as_ref()
                        .expect("Foreign key field not specified");
                    if let Some(field) = fields.iter().find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
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
            matches!(relation.kind, RelationKind::BelongsTo)
                && relation.foreign_key_field.is_some()
                && {
                    // Check if the foreign key field is not nullable (not Option<T>)
                    // Only required relations should be initializers
                    let fk_field_name = relation
                        .foreign_key_field
                        .as_ref()
                        .expect("Foreign key field not specified");
                    if let Some(field) = fields.iter().find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
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

    // Generate unique field names as string literals for match arms
    let unique_field_names: Vec<_> = unique_fields
        .iter()
        .map(|field| {
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            syn::LitStr::new(
                &field_name,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
        })
        .collect();

    // Generate unique field identifiers for column access (PascalCase for SeaORM)
    let unique_field_idents: Vec<_> = unique_fields
        .iter()
        .map(|field| {
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            // Convert to PascalCase for SeaORM Column enum
            let pascal_case = field_name
                .chars()
                .next()
                .expect("Failed to parse relation - this should not happen in valid code")
                .to_uppercase()
                .collect::<String>()
                + &field_name[1..];
            syn::Ident::new(
                &pascal_case,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
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
                let fk_field_name = relation.foreign_key_field.as_ref().expect("Foreign key field not specified");
                if let Some(field) = fields.iter().find(|f| f.ident.as_ref().expect("Field has no identifier").to_string() == *fk_field_name) {
                    !is_option(&field.ty)
                } else {
                    false
                }
            }
        })
        .map(|relation| {
            let fk_field = relation.foreign_key_field.as_ref().expect("Foreign key field not specified");
            let fk_field_ident = format_ident!("{}", fk_field);
            let relation_name = format_ident!("{}", relation.name.to_snake_case());
            let target_module = &relation.target;

            // Add variables for registry-based conversion
            let entity_name = entity_context.registry_name();
            let foreign_key_field_name = fk_field;

            // Get the primary key field name from the relation definition or use dynamic detection
            let primary_key_field_name_raw = if let Some(pk) = &relation.primary_key_field {
                pk.clone()
            } else {
                // Use the current entity's primary key field name
                get_primary_key_field_name(&fields)
            };
            let primary_key_field_name = primary_key_field_name_raw.to_snake_case();
            let primary_key_pascal = primary_key_field_name_raw.chars().next().expect("Primary key field name is empty").to_uppercase().collect::<String>()
                + &primary_key_field_name_raw[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);
            let primary_key_field_ident = format_ident!("{}", primary_key_field_name);

            quote! {
                // Handle foreign key value from UniqueWhereParam
                match self.#relation_name {
                    #target_module::UniqueWhereParam::#primary_key_variant(key) => {
                        // Extract the value from CausticsKey for database field assignment
                        let fk_value = crate::__caustics_convert_key_to_active_value(#entity_name, #foreign_key_field_name, key);
                        model.#fk_field_ident = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
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
                                // Extract the value from CausticsKey for database field assignment
                                let fk_value = crate::__caustics_convert_key_to_active_value(#entity_name, #foreign_key_field_name, value);
                                model.#fk_field_ident = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
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
                                    result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
                                        caustics::CausticsError::NotFoundForCondition {
                                            entity: stringify!(#target_module).to_string(),
                                            condition: format!("{:?}", param),
                                        }.into()
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
                                    result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
                                        caustics::CausticsError::NotFoundForCondition {
                                            entity: stringify!(#target_module).to_string(),
                                            condition: format!("{:?}", param),
                                        }.into()
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
            let name = field.ident.as_ref().expect("Field has no identifier");
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
            let name = field.ident.as_ref().expect("Field has no identifier");
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
                    let fk_field_name = relation
                        .foreign_key_field
                        .as_ref()
                        .expect("Foreign key field not specified");
                    if let Some(field) = fields.iter().find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
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

    // Generate has_many set operation variants for SetParam enum
    let has_many_set_variants = relations
        .iter()
        .filter_map(|relation| match relation.kind {
            RelationKind::HasMany => {
                let relation_name = format_ident!("Set{}", relation.name.to_pascal_case());
                let target_module = &relation.target;
                Some((relation.name.clone(), relation_name, target_module.clone()))
            }
            _ => None,
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

    // Generate has_many create and createMany variants for SetParam enum (nested writes)
    let current_entity_snake = model_ast.ident.to_string().to_snake_case();
    let has_many_create_variants = relations
        .iter()
        .filter_map(|relation| {
            match relation.kind {
                RelationKind::HasMany => {
                    let create_variant = format_ident!("Create{}", relation.name.to_pascal_case());
                    let create_many_variant =
                        format_ident!("CreateMany{}", relation.name.to_pascal_case());
                    // Determine FK field ident on child ActiveModel
                    let fk_field_name = relation
                        .foreign_key_column
                        .clone()
                        .unwrap_or_else(|| format!("{}_id", current_entity_snake.clone()));
                    let fk_field_ident = format_ident!("{}", fk_field_name.to_snake_case());
                    let fk_col_ident_pascal = format_ident!(
                        "{}",
                        fk_field_name
                            .split('_')
                            .map(|part| {
                                let mut chars = part.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => first.to_uppercase().chain(chars).collect(),
                                }
                            })
                            .collect::<String>()
                    );
                    let target_module = &relation.target;
                    // For has_many nested create, set child's FK to parent id which is non-null in our schemas
                    let is_fk_nullable_lit =
                        syn::LitBool::new(false, proc_macro2::Span::call_site());
                    // Table/column literals for handler
                    let relation_name_snake = relation.name.to_snake_case();
                    // Use the resolved target table name from build-time metadata
                    let target_table_name_expr = quote! { #relation_name_snake };
                    let current_table_name = relation
                        .current_table_name
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| {
                            // This should not happen if relations are properly configured
                            panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
                        });
                    let current_primary_key_column = get_primary_key_column_name(&fields);
                    let target_primary_key_column =
                        if let Some(pk_field) = &relation.primary_key_field {
                            pk_field.clone()
                        } else {
                            current_primary_key_column.clone()
                        };
                    let fk_column_lit =
                        syn::LitStr::new(&fk_field_name, proc_macro2::Span::call_site());
                    let target_table_lit = target_table_name_expr;
                    let current_pk_col_lit = syn::LitStr::new(
                        &current_primary_key_column,
                        proc_macro2::Span::call_site(),
                    );
                    let target_pk_col_lit = syn::LitStr::new(
                        &target_primary_key_column,
                        proc_macro2::Span::call_site(),
                    );
                    Some((
                        create_variant,
                        create_many_variant,
                        fk_field_ident,
                        fk_col_ident_pascal,
                        target_module.clone(),
                        is_fk_nullable_lit,
                        fk_column_lit,
                        target_table_lit,
                        current_pk_col_lit,
                        target_pk_col_lit,
                    ))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    // Tokens for enum variants
    let has_many_create_variant_tokens: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(
            |(create_variant, create_many_variant, _, _, target_module, ..)| {
                quote! {
                    #create_variant(Vec<#target_module::Create>),
                    #create_many_variant(Vec<#target_module::Create>)
                }
            },
        )
        .collect();

    // Match arms for nested create
    let has_many_create_match_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(create_variant, _create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_variant(mut items) => {
                    let items_arc_main = std::sync::Arc::new(items.clone());
                    let items_arc_for_conn = std::sync::Arc::clone(&items_arc_main);
                    let items_arc_for_txn = std::sync::Arc::clone(&items_arc_main);
                    let run_conn: Box<
                        dyn for<'b> Fn(
                                &'b sea_orm::DatabaseConnection,
                                caustics::CausticsKey,
                            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>>
                            + Send
                    > = Box::new(move |conn: &sea_orm::DatabaseConnection, parent_id: caustics::CausticsKey| {
                        let items_arc2 = std::sync::Arc::clone(&items_arc_for_conn);
                        Box::pin(async move {
                            let items_local = (*items_arc2).clone();
                            // Use parent_id directly with to_db_value()
                            for c in items_local.iter() {
                                let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseConnection>();
                                let lookups: Vec<_> = child_lookups.iter().collect();
                                for lookup in lookups {
                                    let v = (lookup.resolve_on_conn)(conn, &*lookup.unique_param).await?;
                                    (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                                }
                                // Set the foreign key to the parent id before insert
                                child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                                let inserted_child = child_am.insert(conn).await?;
                                let child_id = #target_module::__extract_id(&inserted_child);
                                for op in child_post_ops {
                                    (op.run_on_conn)(conn, child_id.clone()).await?;
                                }
                            }
                            Ok(())
                        })
                    });
                    let run_txn: Box<
                        dyn for<'b> Fn(
                                &'b sea_orm::DatabaseTransaction,
                                caustics::CausticsKey,
                            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>>
                            + Send
                    > = Box::new(move |txn: &sea_orm::DatabaseTransaction, parent_id: caustics::CausticsKey| {
                        let items_arc3 = std::sync::Arc::clone(&items_arc_for_txn);
                        Box::pin(async move {
                            let items_local = (*items_arc3).clone();
                            // Use parent_id directly with to_db_value()
                            for c in items_local.iter() {
                                let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseTransaction>();
                                let lookups: Vec<_> = child_lookups.iter().collect();
                                for lookup in lookups {
                                    let v = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
                                    (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                                }
                                // Set the foreign key to the parent id before insert
                                child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                                let inserted_child = child_am.insert(txn).await?;
                                let child_id = #target_module::__extract_id(&inserted_child);
                                for op in child_post_ops {
                                    (op.run_on_txn)(txn, child_id.clone()).await?;
                                }
                            }
                            Ok(())
                        })
                    });
                    post_insert_ops.push(caustics::PostInsertOp { run_on_conn: run_conn, run_on_txn: run_txn });
                }
            }
        })
        .collect();

    // Match arms for nested createMany
    let has_many_create_many_match_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(_create_variant, create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_many_variant(mut items) => {
                    let items_arc_main = std::sync::Arc::new(items.clone());
                    let items_arc_for_conn = std::sync::Arc::clone(&items_arc_main);
                    let items_arc_for_txn = std::sync::Arc::clone(&items_arc_main);
                    let run_conn: Box<
                        dyn for<'b> Fn(
                                &'b sea_orm::DatabaseConnection,
                                caustics::CausticsKey,
                            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>>
                            + Send
                    > = Box::new(move |conn: &sea_orm::DatabaseConnection, parent_id: caustics::CausticsKey| {
                        let items_arc2 = std::sync::Arc::clone(&items_arc_for_conn);
                        Box::pin(async move {
                            let items_local = (*items_arc2).clone();
                            // Use parent_id directly with to_db_value()
                            for c in items_local.iter() {
                                let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseConnection>();
                                let lookups: Vec<_> = child_lookups.iter().collect();
                                for lookup in lookups {
                                    let v = (lookup.resolve_on_conn)(conn, &*lookup.unique_param).await?;
                                    (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                                }
                                // Set the foreign key to the parent id before insert
                                child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                                let inserted_child = child_am.insert(conn).await?;
                                let child_id = #target_module::__extract_id(&inserted_child);
                                for op in child_post_ops {
                                    (op.run_on_conn)(conn, child_id.clone()).await?;
                                }
                            }
                            Ok(())
                        })
                    });
                    let run_txn: Box<
                        dyn for<'b> Fn(
                                &'b sea_orm::DatabaseTransaction,
                                caustics::CausticsKey,
                            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>>
                            + Send
                    > = Box::new(move |txn: &sea_orm::DatabaseTransaction, parent_id: caustics::CausticsKey| {
                        let items_arc3 = std::sync::Arc::clone(&items_arc_for_txn);
                        Box::pin(async move {
                            let items_local = (*items_arc3).clone();
                            // Use parent_id directly with to_db_value()
                            for c in items_local.iter() {
                                let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseTransaction>();
                                let lookups: Vec<_> = child_lookups.iter().collect();
                                for lookup in lookups {
                                    let v = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
                                    (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                                }
                                // Set the foreign key to the parent id before insert
                                child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                                let inserted_child = child_am.insert(txn).await?;
                                let child_id = #target_module::__extract_id(&inserted_child);
                                for op in child_post_ops {
                                    (op.run_on_txn)(txn, child_id.clone()).await?;
                                }
                            }
                            Ok(())
                        })
                    });
                    post_insert_ops.push(caustics::PostInsertOp { run_on_conn: run_conn, run_on_txn: run_txn });
                }
            }
        })
        .collect();

    // Flag match arms to detect create/createMany on update
    let has_many_create_flag_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(create_variant, _create_many_variant, ..)| {
            quote! { SetParam::#create_variant(_) => true, }
        })
        .collect();

    let has_many_create_many_flag_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(_create_variant, create_many_variant, ..)| {
            quote! { SetParam::#create_many_variant(_) => true, }
        })
        .collect();

    // Exec arms for nested create on update (connection)
    let has_many_create_exec_conn_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(create_variant, _create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_variant(items) => {
                    let items_local = items.clone();
                    // Use parent_id directly with to_db_value()
                    for c in items_local.iter() {
                        let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseConnection>();
                        let lookups: Vec<_> = child_lookups.iter().collect();
                        for lookup in lookups {
                            let v = (lookup.resolve_on_conn)(conn, &*lookup.unique_param).await?;
                            (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                        }
                        child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                        let inserted_child = child_am.insert(conn).await?;
                        let child_id = #target_module::__extract_id(&inserted_child);
                        for op in child_post_ops { (op.run_on_conn)(conn, child_id.clone()).await?; }
                    }
                    Ok(())
                }
            }
        })
        .collect();

    let has_many_create_many_exec_conn_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(_create_variant, create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_many_variant(items) => {
                    let items_local = items.clone();
                    // Use parent_id directly with to_db_value()
                    for c in items_local.iter() {
                        let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseConnection>();
                        let lookups: Vec<_> = child_lookups.iter().collect();
                        for lookup in lookups {
                            let v = (lookup.resolve_on_conn)(conn, &*lookup.unique_param).await?;
                            (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                        }
                        child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                        let inserted_child = child_am.insert(conn).await?;
                        let child_id = #target_module::__extract_id(&inserted_child);
                        for op in child_post_ops { (op.run_on_conn)(conn, child_id.clone()).await?; }
                    }
                    Ok(())
                }
            }
        })
        .collect();

    // Exec arms for nested create on update (transaction)
    let has_many_create_exec_txn_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(create_variant, _create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_variant(items) => {
                    let items_local = items.clone();
                    for c in items_local.iter() {
                        let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseTransaction>();
                        let lookups: Vec<_> = child_lookups.iter().collect();
                        for lookup in lookups {
                            let v = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
                            (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                        }
                        // Use parent_id directly with to_db_value()
                        child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                        let inserted_child = child_am.insert(txn).await?;
                        let child_id = #target_module::__extract_id(&inserted_child);
                        for op in child_post_ops { (op.run_on_txn)(txn, child_id.clone()).await?; }
                    }
                    Ok(())
                }
            }
        })
        .collect();

    let has_many_create_many_exec_txn_arms: Vec<proc_macro2::TokenStream> = has_many_create_variants
        .iter()
        .map(|(_create_variant, create_many_variant, _fk_field_ident, fk_col_ident_pascal, target_module, _is_fk_nullable_lit, _fk_column_lit, _target_table_lit, _current_pk_col_lit, _target_pk_col_lit)| {
            quote! {
                SetParam::#create_many_variant(items) => {
                    let items_local = items.clone();
                    for c in items_local.iter() {
                        let (mut child_am, child_lookups, child_post_ops) = c.clone().into_active_model::<sea_orm::DatabaseTransaction>();
                        let lookups: Vec<_> = child_lookups.iter().collect();
                        for lookup in lookups {
                            let v = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
                            (lookup.assign)(&mut child_am as &mut (dyn std::any::Any + 'static), v);
                        }
                        // Use parent_id directly with to_db_value()
                        child_am.set(<#target_module::Entity as sea_orm::EntityTrait>::Column::#fk_col_ident_pascal, parent_id.to_db_value());
                        let inserted_child = child_am.insert(txn).await?;
                        let child_id = #target_module::__extract_id(&inserted_child);
                        for op in child_post_ops { (op.run_on_txn)(txn, child_id.clone()).await?; }
                    }
                    Ok(())
                }
            }
        })
        .collect();

    // Combine all SetParam variants as a flat Vec
    let all_set_param_variants: Vec<_> = field_variants
        .clone()
        .into_iter()
        .chain(atomic_variants)
        .chain(relation_connect_variants)
        .chain(relation_disconnect_variants)
        .chain(has_many_set_variant_tokens)
        .chain(has_many_create_variant_tokens)
        .collect();

    // Generate field variants and field operator modules for WhereParam enum (all fields, with string ops for string fields)
    let primary_key_fields_slice: Vec<&syn::Field> =
        primary_key_fields.iter().map(|f| **f).collect();
    let (where_field_variants, where_match_arms, field_ops) = generate_where_param_logic(
        &fields,
        &unique_fields,
        &primary_key_fields_slice,
        full_mod_path,
        &relations,
        entity_context.registry_name(),
    );

    // Generate match arms for UniqueWhereParam
    let unique_where_match_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let equals_variant = format_ident!("{}Equals", pascal_name);

            // For primary keys, use CausticsKey and convert using registry
            // For other unique fields, use the field value directly
            if primary_key_fields.contains(&field) {
                let field_name_snake = name.to_string().to_snake_case();
                let entity_name_str = &entity_name;
                quote! {
                    UniqueWhereParam::#equals_variant(key) => {
                        let value = crate::__caustics_convert_key_for_sea_orm(#entity_name_str, #field_name_snake, key)
                            .expect("Failed to convert CausticsKey to field type");
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(value))
                    }
                }
            } else {
                quote! {
                    UniqueWhereParam::#equals_variant(value) => {
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(value))
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate match arms to convert UniqueWhereParam into a cursor (expr, value)
    // Each arm evaluates to a new builder (Self)
    let unique_cursor_match_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let equals_variant = format_ident!("{}Equals", pascal_name);

            // For primary keys, use CausticsKey and convert using registry
            // For other unique fields, use the field value directly
            if primary_key_fields.contains(&field) {
                let field_name_snake = name.to_string().to_snake_case();
                let entity_name_str = &entity_name;
                quote! {
                    UniqueWhereParam::#equals_variant(key) => {
                        let value = crate::__caustics_convert_key_for_sea_orm(#entity_name_str, #field_name_snake, key)
                            .expect("Failed to convert CausticsKey to field type");
                        let expr = <Entity as EntityTrait>::Column::#pascal_name.into_simple_expr();
                        self.with_cursor(expr, sea_orm::Value::from(value))
                    },
                }
            } else {
                quote! {
                    UniqueWhereParam::#equals_variant(value) => {
                        let expr = <Entity as EntityTrait>::Column::#pascal_name.into_simple_expr();
                        self.with_cursor(expr, value.into())
                    },
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate parallel lists of equals-variants and their columns for Into<(expr, value)>
    let unique_where_equals_variants = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            format_ident!("{}Equals", pascal_name)
        })
        .collect::<Vec<_>>();

    let unique_where_equals_columns = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            pascal_name
        })
        .collect::<Vec<_>>();

    // Generate match arms for From<UniqueWhereParam> for (sea_query::SimpleExpr, sea_orm::Value)
    // Handle primary keys and other unique fields differently
    let unique_where_to_expr_value_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let variant = format_ident!("{}Equals", pascal_name);
            let column = pascal_name.clone();

            if primary_key_fields.contains(&field) {
                // For primary keys, value is CausticsKey, convert using registry
                let field_name_snake = name.to_string().to_snake_case();
                let entity_name_str = &entity_name;
                quote! {
                    UniqueWhereParam::#variant(key) => {
                        let value = crate::__caustics_convert_key_for_sea_orm(#entity_name_str, #field_name_snake, key)
                            .expect("Failed to convert CausticsKey to field type");
                        let expr = <Entity as EntityTrait>::Column::#column.into_simple_expr();
                        (expr, sea_orm::Value::from(value))
                    }
                }
            } else {
                // For other unique fields, value is the field's actual type, use From<T>
                quote! {
                    UniqueWhereParam::#variant(value) => {
                        let expr = <Entity as EntityTrait>::Column::#column.into_simple_expr();
                        (expr, sea_orm::Value::from(value))
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate field variants for OrderByParam enum (all fields)
    let order_by_field_variants = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
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
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .to_string()
                    .to_pascal_case()
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
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .to_string()
                    .to_pascal_case()
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
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .to_string()
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
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .to_string()
                    .to_pascal_case()
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
    let sum_select_variants = fields
        .iter()
        .map(|field| {
            let pascal_name = format_ident!(
                "{}",
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .to_string()
                    .to_pascal_case()
            );
            quote! { #pascal_name }
        })
        .collect::<Vec<_>>();
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
                        let field = ToString::to_string(&#field_name_lit);
                        let operation = match op {
                            caustics::FieldOp::Equals(v) => match v { Some(v) => caustics::FieldOp::Equals(ToString::to_string(&v)), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::NotEquals(v) => match v { Some(v) => caustics::FieldOp::NotEquals(ToString::to_string(&v)), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Gt(v) => match v { Some(v) => caustics::FieldOp::Gt(ToString::to_string(&v)), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Lt(v) => match v { Some(v) => caustics::FieldOp::Lt(ToString::to_string(&v)), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::Gte(v) => match v { Some(v) => caustics::FieldOp::Gte(ToString::to_string(&v)), None => caustics::FieldOp::IsNotNull },
                            caustics::FieldOp::Lte(v) => match v { Some(v) => caustics::FieldOp::Lte(ToString::to_string(&v)), None => caustics::FieldOp::IsNull },
                            caustics::FieldOp::InVec(vs) => caustics::FieldOp::InVec(vs.into_iter().filter_map(|v| v.map(|x| ToString::to_string(&x))).collect()),
                            caustics::FieldOp::NotInVec(vs) => caustics::FieldOp::NotInVec(vs.into_iter().filter_map(|v| v.map(|x| ToString::to_string(&x))).collect()),
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
                            caustics::FieldOp::JsonNull(flag) => caustics::FieldOp::JsonNull(flag),
                            caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => {
                                // These operations are not supported in this context
                                continue;
                            },
                        };
                        caustics::Filter { field, operation }
                    }
                }
            } else {
                quote! {
                    WhereParam::#pascal_name(op) => {
                        let field = ToString::to_string(&#field_name_lit);
                        let operation = match op {
                            caustics::FieldOp::Equals(v) => caustics::FieldOp::Equals(ToString::to_string(&v)),
                            caustics::FieldOp::NotEquals(v) => caustics::FieldOp::NotEquals(ToString::to_string(&v)),
                            caustics::FieldOp::Gt(v) => caustics::FieldOp::Gt(ToString::to_string(&v)),
                            caustics::FieldOp::Lt(v) => caustics::FieldOp::Lt(ToString::to_string(&v)),
                            caustics::FieldOp::Gte(v) => caustics::FieldOp::Gte(ToString::to_string(&v)),
                            caustics::FieldOp::Lte(v) => caustics::FieldOp::Lte(ToString::to_string(&v)),
                            caustics::FieldOp::InVec(vs) => caustics::FieldOp::InVec(vs.into_iter().map(|v| ToString::to_string(&v)).collect()),
                            caustics::FieldOp::NotInVec(vs) => caustics::FieldOp::NotInVec(vs.into_iter().map(|v| ToString::to_string(&v)).collect()),
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
                            caustics::FieldOp::JsonNull(flag) => caustics::FieldOp::JsonNull(flag),
                            caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => {
                                // These operations are not supported in this context
                                continue;
                            },
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
            let name = field.ident.as_ref().expect("Field has no identifier");
            let ty = &field.ty;
            let pascal_name = name.to_string().to_pascal_case();
            let equals_variant = format_ident!("{}Equals", pascal_name);

            // Only use CausticsKey for primary key fields, otherwise use the field's actual type
            if primary_key_fields.contains(&field) {
                quote! {
                    #equals_variant(caustics::CausticsKey)
                }
            } else {
                quote! {
                    #equals_variant(#ty)
                }
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
        .filter(|ident| *ident != "IdEquals")
        .collect();

    // Generate UniqueWhereParam serialize implementation
    let unique_where_serialize_arms = unique_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let pascal_name = name.to_string().to_pascal_case();
            let equals_variant = format_ident!("{}Equals", pascal_name);
            let field_name = name.to_string();

            // For primary keys, use CausticsKey and convert to Int
            // For other unique fields, use the field value directly
            if primary_key_fields.contains(&field) {
                let field_name_snake = name.to_string().to_snake_case();
                let entity_name_str = &entity_name;
                quote! {
                    UniqueWhereParam::#equals_variant(key) => {
                        let value = crate::__caustics_convert_key_for_sea_orm(#entity_name_str, #field_name_snake, key)
                            .expect("Failed to convert CausticsKey to field type");
                        (
                            #field_name,
                            ::prisma_client_rust::SerializedWhereValue::Value(
                                sea_orm::Value::from(value).into(),
                            ),
                        )
                    },
                }
            } else {
                quote! {
                    UniqueWhereParam::#equals_variant(value) => (
                        #field_name,
                        ::prisma_client_rust::SerializedWhereValue::Value(
                            ::prisma_client_rust::PrismaValue::String(value.to_string()),
                        ),
                    ),
                }
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
                        // Try to downcast as Vec<ModelWithRelations> first (no field selection)
                        if let Some(vec_ref) = fetched_result.downcast_mut::<Option<Vec<#target::ModelWithRelations>>>() {
                            if let Some(vec_inner) = vec_ref.as_mut() {
                                for elem in vec_inner.iter_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(elem, conn, nested, registry).await?;
                                    }
                                }
                            }
                        } else if let Some(vec_ref) = fetched_result.downcast_mut::<Option<Vec<#target::Selected>>>() {
                            // Try to downcast as Vec<Selected> (field selection is being used)
                            if let Some(vec_inner) = vec_ref.as_mut() {
                                for elem in vec_inner.iter_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(elem, conn, nested, registry).await?;
                                    }
                                }
                            }
                        } else {
                            panic!("Type mismatch in nested has_many downcast: expected Option<Vec<{}>> or Option<Vec<{}>>",
                                stringify!(#target::ModelWithRelations), stringify!(#target::Selected));
                        }
                    }
                }
                RelationKind::BelongsTo => {
                    // Determine optional vs required
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields
                            .iter()
                            .find(|f| f.ident.as_ref().expect("Field has no identifier").to_string() == *fk_field_name)
                        {
                            is_option(&field.ty)
                        } else { false }
                    } else { false };
                    if is_optional {
                        quote! {
                            // Try to downcast as Option<ModelWithRelations> first (no field selection)
                            if let Some(mmref) = fetched_result.downcast_mut::<Option<#target::ModelWithRelations>>() {
                                if let Some(model) = mmref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(mmref) = fetched_result.downcast_mut::<Option<#target::Selected>>() {
                                // Try to downcast as Option<Selected> (field selection is being used)
                                if let Some(model) = mmref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else {
                                panic!("Type mismatch in nested optional belongs_to downcast: expected Option<{}> or Option<{}>",
                                    stringify!(#target::ModelWithRelations), stringify!(#target::Selected));
                            }
                        }
                    } else {
                        quote! {
                            // Try to downcast as ModelWithRelations first (no field selection)
                            if let Some(mref) = fetched_result.downcast_mut::<Option<#target::ModelWithRelations>>() {
                                if let Some(model) = mref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(model) = fetched_result.downcast_mut::<#target::ModelWithRelations>() {
                                for nested in &filter.nested_includes {
                                    #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                }
                            } else if let Some(mref) = fetched_result.downcast_mut::<Option<#target::Selected>>() {
                                // Try to downcast as Selected (field selection is being used)
                                if let Some(model) = mref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(model) = fetched_result.downcast_mut::<#target::Selected>() {
                                for nested in &filter.nested_includes {
                                    #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                }
                            } else {
                                panic!("Type mismatch in nested belongs_to downcast: expected {} or {} or Option<{}> or Option<{}>",
                                    stringify!(#target::ModelWithRelations), stringify!(#target::Selected),
                                    stringify!(#target::ModelWithRelations), stringify!(#target::Selected));
                            }
                        }
                    }
                }
            }
        })
        .collect();

    // Generate nested apply blocks for Selected types
    let selected_relation_nested_apply_blocks: Vec<_> = relations
        .iter()
        .map(|relation| {
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => {
                    quote! {
                        // Try to downcast as Vec<Selected> first (field selection is being used)
                        if let Some(vec_ref) = fetched_result.downcast_mut::<Option<Vec<#target::Selected>>>() {
                            if let Some(vec_inner) = vec_ref.as_mut() {
                                for elem in vec_inner.iter_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(elem, conn, nested, registry).await?;
                                    }
                                }
                            }
                        } else if let Some(vec_ref) = fetched_result.downcast_mut::<Option<Vec<#target::ModelWithRelations>>>() {
                            // Try to downcast as Vec<ModelWithRelations> (no field selection)
                            if let Some(vec_inner) = vec_ref.as_mut() {
                                for elem in vec_inner.iter_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(elem, conn, nested, registry).await?;
                                    }
                                }
                            }
                        } else {
                            panic!("Type mismatch in nested has_many downcast: expected Option<Vec<{}>> or Option<Vec<{}>>",
                                stringify!(#target::Selected), stringify!(#target::ModelWithRelations));
                        }
                    }
                }
                RelationKind::BelongsTo => {
                    // Determine optional vs required
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields
                            .iter()
                            .find(|f| f.ident.as_ref().expect("Field has no identifier").to_string() == *fk_field_name)
                        {
                            is_option(&field.ty)
                        } else { false }
                    } else { false };
                    if is_optional {
                        quote! {
                            // Try to downcast as Option<Selected> first (field selection is being used)
                            if let Some(mmref) = fetched_result.downcast_mut::<Option<#target::Selected>>() {
                                if let Some(model) = mmref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(mmref) = fetched_result.downcast_mut::<Option<#target::ModelWithRelations>>() {
                                // Try to downcast as Option<ModelWithRelations> (no field selection)
                                if let Some(model) = mmref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else {
                                panic!("Type mismatch in nested optional belongs_to downcast: expected Option<{}> or Option<{}>",
                                    stringify!(#target::Selected), stringify!(#target::ModelWithRelations));
                            }
                        }
                    } else {
                        quote! {
                            // Try to downcast as Selected first (field selection is being used)
                            if let Some(mref) = fetched_result.downcast_mut::<Option<#target::Selected>>() {
                                if let Some(model) = mref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(model) = fetched_result.downcast_mut::<#target::Selected>() {
                                for nested in &filter.nested_includes {
                                    #target::Selected::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                }
                            } else if let Some(mref) = fetched_result.downcast_mut::<Option<#target::ModelWithRelations>>() {
                                // Try to downcast as ModelWithRelations (no field selection)
                                if let Some(model) = mref.as_mut() {
                                    for nested in &filter.nested_includes {
                                        #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                    }
                                }
                            } else if let Some(model) = fetched_result.downcast_mut::<#target::ModelWithRelations>() {
                                for nested in &filter.nested_includes {
                                    #target::ModelWithRelations::__caustics_apply_relation_filter(model, conn, nested, registry).await?;
                                }
                            } else {
                                panic!("Type mismatch in nested belongs_to downcast: expected {} or {} or Option<{}> or Option<{}>",
                                    stringify!(#target::Selected), stringify!(#target::ModelWithRelations),
                                    stringify!(#target::Selected), stringify!(#target::ModelWithRelations));
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
            let name = field.ident.as_ref().expect("Field has no identifier");
            let ty = &field.ty;
            quote! { pub #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate field names for From implementation
    let field_names = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate field names and types for constructor
    let field_params = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
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
                        if let Some(field) = fields.iter().find(|f| {
                            f.ident
                                .as_ref()
                                .expect("Field has no identifier")
                                .to_string()
                                == *fk_field_name
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
                            f.ident
                                .as_ref()
                                .expect("Field has no identifier")
                                .to_string()
                                == *fk_field_name
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
            pub cursor_id: Option<caustics::CausticsKey>,
            pub include_count: bool,
            pub distinct: bool,
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
                    distinct: relation_filter.distinct,
                }
            }
        }
    };

    // Prepare Selected scalar field definitions (Option<T> for non-nullable, Option<Option<T>> for nullable)
    let selected_scalar_fields = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let original_ty = &field.ty;
            let inner_ty = crate::common::extract_inner_type_from_option(&field.ty);

            // Check if the original type is Option<T> (nullable) or T (non-nullable)
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<Option<InnerType>> - first Option for "fetched?", second for "null?"
                quote! { pub #name: Option<Option<#inner_ty>> }
            } else {
                // For non-nullable fields: Option<InnerType> - Option for "fetched?"
                quote! { pub #name: Option<#inner_ty> }
            }
        })
        .collect::<Vec<_>>();

    // Generate field names for from_model function
    let field_names = fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect::<Vec<_>>();

    // Generate field metadata for FromQueryResult implementation
    let field_inner_types = fields
        .iter()
        .map(|field| crate::common::extract_inner_type_from_option(&field.ty))
        .collect::<Vec<_>>();

    let field_is_nullable = fields
        .iter()
        .map(|field| crate::common::is_option(&field.ty))
        .collect::<Vec<_>>();

    // Generate relation field names for to_model_with_relations function
    let relation_names = relations
        .iter()
        .map(|relation| format_ident!("{}", relation.name.to_snake_case()))
        .collect::<Vec<_>>();

    // Generate per-field row extraction statements using snake_case aliases
    // Only extract fields that were actually selected (present in the fields parameter)
    let selected_fill_stmts = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let inner_ty = crate::common::extract_inner_type_from_option(&field.ty);
            let alias = syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<Option<InnerType>> - first Option for "fetched?", second for "null?"
                quote! {
                    if fields.contains(&stringify!(#name)) || stringify!(#name) == stringify!(#current_primary_key_ident) {
                        s.#name = Some(row.try_get::<#inner_ty>("", #alias).ok());
                    }
                }
            } else {
                // For non-nullable fields: Option<InnerType> - Option for "fetched?"
                quote! {
                    if fields.contains(&stringify!(#name)) || stringify!(#name) == stringify!(#current_primary_key_ident) {
                        s.#name = row.try_get::<#inner_ty>("", #alias).ok();
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation fields for Selected struct (using Selected types)
    let selected_relation_fields = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => {
                    quote! { pub #name: Option<Vec<#target::Selected>> }
                }
                RelationKind::BelongsTo => {
                    // Check if this is an optional relation by looking at the foreign key field
                    let is_optional = if let Some(fk_field_name) = &relation.foreign_key_field {
                        if let Some(field) = fields.iter().find(|f| {
                            f.ident
                                .as_ref()
                                .expect("Field has no identifier")
                                .to_string()
                                == *fk_field_name
                        }) {
                            is_option(&field.ty)
                        } else {
                            false
                        }
                    } else {
                        relation.is_nullable
                    };

                    if is_optional {
                        quote! { pub #name: Option<Option<#target::Selected>> }
                    } else {
                        quote! { pub #name: Option<#target::Selected> }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    // clear_unselected method no longer needed - fields are only populated if they were selected

    // Match arms for get_key for all primary key and foreign key fields
    let get_key_match_arms = fields
        .iter()
        .filter(|field| {
            let field_name = field.ident.as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            // Include primary key fields and foreign key fields
            primary_key_fields.contains(field) || foreign_key_fields.contains(&field_name)
        })
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let alias = syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<T> -> Option<CausticsKey>
                quote! {
                    #alias => self.#name.as_ref().and_then(|v| caustics::CausticsKey::from_db_value(&v.clone().into()))
                }
            } else {
                // For non-nullable fields: T -> Option<CausticsKey>
                quote! {
                    #alias => caustics::CausticsKey::from_db_value(&self.#name.clone().into())
                }
            }
        })
        .collect::<Vec<_>>();

    // Prepare alias/id pairs for Selected::column_for_alias
    let selected_all_field_names: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            syn::LitStr::new(
                &field_name,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
        })
        .collect();
    let selected_all_field_idents: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
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
            syn::Ident::new(
                &pascal_case,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
        })
        .collect();

    // Snake_case field idents for Selected struct field access (available early)
    let selected_field_idents_snake: Vec<syn::Ident> = fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap().clone())
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

    // Relation-aggregate orderBy support: variants and match arms
    let relation_order_by_variants = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let variant = format_ident!("{}Count", relation.name.to_pascal_case());
                Some(quote! { #variant(caustics::SortOrder) })
            } else if matches!(relation.kind, RelationKind::BelongsTo) {
                // For belongs_to relations, we need to support field ordering
                // This is more complex as it requires subqueries
                let variant = format_ident!("{}Field", relation.name.to_pascal_case());
                Some(quote! { #variant(String, caustics::SortOrder) })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Function names for the order_by sugar: e.g., enrollments_count(order)
    let relation_order_by_fn_names: Vec<syn::Ident> = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let fn_name = format_ident!("{}_count", relation.name.to_snake_case());
                Some(fn_name)
            } else {
                None
            }
        })
        .collect();

    // Corresponding enum variant idents to construct
    let relation_order_by_fn_variants: Vec<syn::Ident> = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let variant = format_ident!("{}Count", relation.name.to_pascal_case());
                Some(variant)
            } else {
                None
            }
        })
        .collect();

    // Determine current entity primary key column name (snake_case field name)
    let current_pk_column_name: String = get_primary_key_column_name(&fields);

    let relation_order_match_arms_many = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let variant = format_ident!("{}Count", relation.name.to_pascal_case());
                // Use the resolved target table name from build-time metadata
                let relation_name_snake = relation.name.to_snake_case();
                let target_table_name_expr = quote! { #relation_name_snake };
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
                    });
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let target_table_lit = target_table_name_expr;
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let pk_col_lit = syn::LitStr::new(&current_pk_column_name, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT COUNT(*) FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            #target_table_lit, #target_table_lit, #fk_col_lit, #current_table_lit, #pk_col_lit
                        ));
                        self.pending_order_bys.push((expr, sea_order));
                    }
                })
            } else if matches!(relation.kind, RelationKind::BelongsTo) {
                let variant = format_ident!("{}Field", relation.name.to_pascal_case());
                let relation_name_snake = relation.name.to_snake_case();
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        entity_name.to_snake_case()
                    });
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let target_table_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let target_pk_col = relation.primary_key_field.as_ref().unwrap_or(&relation_name_snake);
                let target_pk_col_lit = syn::LitStr::new(target_pk_col, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(field_name, order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT \"{}\" FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            field_name, #target_table_lit, #target_table_lit, #target_pk_col_lit, #current_table_lit, #fk_col_lit
                        ));
                        self.pending_order_bys.push((expr, sea_order));
                    }
                })
            } else { None }
        })
        .collect::<Vec<_>>();

    let relation_order_match_arms_select_many = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let variant = format_ident!("{}Count", relation.name.to_pascal_case());
                // Use the resolved target table name from build-time metadata
                let relation_name_snake = relation.name.to_snake_case();
                let target_table_name_expr = quote! { #relation_name_snake };
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
                    });
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let target_table_lit = target_table_name_expr;
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let pk_col_lit = syn::LitStr::new(&current_pk_column_name, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT COUNT(*) FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            #target_table_lit, #target_table_lit, #fk_col_lit, #current_table_lit, #pk_col_lit
                        ));
                        self.pending_order_bys.push((expr, sea_order));
                    }
                })
            } else if matches!(relation.kind, RelationKind::BelongsTo) {
                let variant = format_ident!("{}Field", relation.name.to_pascal_case());
                let relation_name_snake = relation.name.to_snake_case();
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        entity_name.to_snake_case()
                    });
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let target_table_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let target_pk_col = relation.primary_key_field.as_ref().unwrap_or(&relation_name_snake);
                let target_pk_col_lit = syn::LitStr::new(target_pk_col, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(field_name, order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT \"{}\" FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            field_name, #target_table_lit, #target_table_lit, #target_pk_col_lit, #current_table_lit, #fk_col_lit
                        ));
                        self.pending_order_bys.push((expr, sea_order));
                    }
                })
            } else { None }
        })
        .collect::<Vec<_>>();

    // Arms returning (expr, order) for IntoOrderByExpr impl
    let relation_order_into_expr_arms = relations
        .iter()
        .filter_map(|relation| {
            if matches!(relation.kind, RelationKind::HasMany) {
                let variant = format_ident!("{}Count", relation.name.to_pascal_case());
                // Use the resolved target table name from build-time metadata
                let relation_name_snake = relation.name.to_snake_case();
                let target_table_name_expr = quote! { #relation_name_snake };
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
                    });
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let target_table_lit = target_table_name_expr;
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let pk_col_lit = syn::LitStr::new(&current_pk_column_name, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT COUNT(*) FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            #target_table_lit, #target_table_lit, #fk_col_lit, #current_table_lit, #pk_col_lit
                        ));
                        (expr, sea_order)
                    }
                })
            } else if matches!(relation.kind, RelationKind::BelongsTo) {
                let variant = format_ident!("{}Field", relation.name.to_pascal_case());
                let relation_name_snake = relation.name.to_snake_case();
                let current_table_name = relation
                    .current_table_name.clone()
                    .unwrap_or_else(|| {
                        entity_name.to_snake_case()
                    });
                let current_table_lit = syn::LitStr::new(&current_table_name, proc_macro2::Span::call_site());
                let target_table_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
                let fk_col_snake = relation
                    .foreign_key_column
                    .as_ref()
                    .map(|s| s.to_snake_case())
                    .unwrap_or_else(|| current_pk_column_name.clone());
                let fk_col_lit = syn::LitStr::new(&fk_col_snake, proc_macro2::Span::call_site());
                let target_pk_col = relation.primary_key_field.as_ref().unwrap_or(&relation_name_snake);
                let target_pk_col_lit = syn::LitStr::new(target_pk_col, proc_macro2::Span::call_site());
                Some(quote! {
                    RelationOrderByParam::#variant(field_name, order) => {
                        let sea_order = match order { caustics::SortOrder::Asc => sea_orm::Order::Asc, _ => sea_orm::Order::Desc };
                        let expr = sea_orm::sea_query::Expr::cust(&format!(
                            "(SELECT \"{}\" FROM \"{}\" WHERE \"{}\".\"{}\" = \"{}\".\"{}\")",
                            field_name, #target_table_lit, #target_table_lit, #target_pk_col_lit, #current_table_lit, #fk_col_lit
                        ));
                        (expr, sea_order)
                    }
                })
            } else { None }
        })
        .collect::<Vec<_>>();

    // Precompute per-relation count arms used inside __caustics_apply_relation_filter (ModelWithRelations)
    let relation_count_match_arms = relations
        .iter()
        .filter_map(|relation| {
            let relation_name_snake = relation.name.to_snake_case();
            let relation_name_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => Some({
                    let foreign_key_column = match validate_foreign_key_column(
                        &relation.name,
                        &relation.foreign_key_column,
                        proc_macro2::Span::call_site(),
                    ) {
                        Ok(col) => col,
                        Err(_) => return None, // Skip this relation if validation fails
                    };
                    let foreign_key_column_ident = format_ident!("{}", foreign_key_column.to_pascal_case());
                    let count_field_ident = format_ident!("{}", relation.name.to_snake_case());
                    quote! {
                        #relation_name_lit => {
                            if let Some(fkv) = foreign_key_value {
                                // Build a count query applying the same filter semantics as the fetcher (ignoring pagination)
                                let mut query = #target::Entity::find()
                                    .filter(#target::Column::#foreign_key_column_ident.eq(fkv));

                                if !filter.filters.is_empty() {
                                    let mut cond = Condition::all();
                                    for f in &filter.filters {
                                        if let Some(col) = #target::column_from_str(&f.field) {
                                            use sea_orm::IntoSimpleExpr;
                                            let col_expr = col.into_simple_expr();
                                            match &f.operation {
                                                caustics::FieldOp::Equals(v) => {
                                                    let val = sea_orm::Value::from(v);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).eq(val));
                                                }
                                                caustics::FieldOp::NotEquals(v) => {
                                                    let val = sea_orm::Value::from(v);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).ne(val));
                                                }
                                                caustics::FieldOp::Contains(s) => {
                                                    let pat = format!("%{}%", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::StartsWith(s) => {
                                                    let pat = format!("{}%", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::EndsWith(s) => {
                                                    let pat = format!("%{}", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::IsNull => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).is_null());
                                                }
                                                caustics::FieldOp::IsNotNull => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).is_not_null());
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    query = query.filter(cond);
                                }

                                if filter.distinct {
                                    query = query.distinct();
                                }

                                let total = query.count(conn).await? as i64;
                                let mut c = self._count.take().unwrap_or_default();
                                c.#count_field_ident = Some(total as i32);
                                self._count = Some(c);
                            }
                        }
                    }
                }),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    // Precompute per-relation count arms for Selected that use generic Value equality
    let relation_count_match_arms_selected = relations
        .iter()
        .filter_map(|relation| {
            let relation_name_snake = relation.name.to_snake_case();
            let relation_name_lit = syn::LitStr::new(&relation_name_snake, proc_macro2::Span::call_site());
            let target = &relation.target;
            match relation.kind {
                RelationKind::HasMany => Some({
                    let foreign_key_column = match validate_foreign_key_column(
                        &relation.name,
                        &relation.foreign_key_column,
                        proc_macro2::Span::call_site(),
                    ) {
                        Ok(col) => col,
                        Err(_) => return None, // Skip this relation if validation fails
                    };
                    let foreign_key_column_ident = format_ident!("{}", foreign_key_column.to_pascal_case());
                    let count_field_ident = format_ident!("{}", relation.name.to_snake_case());
                    quote! {
                        #relation_name_lit => {
                            if let Some(fkv) = foreign_key_value_any.clone() {
                                // Build a count query applying the same filter semantics as the fetcher (ignoring pagination)
                                let col_expr = <#target::Entity as sea_orm::EntityTrait>::Column::#foreign_key_column_ident.into_simple_expr();
                                let mut query = #target::Entity::find()
                                    .filter(Expr::expr(col_expr).eq(fkv));

                                if !filter.filters.is_empty() {
                                    let mut cond = Condition::all();
                                    for f in &filter.filters {
                                        if let Some(col) = #target::column_from_str(&f.field) {
                                            use sea_orm::IntoSimpleExpr;
                                            let col_expr = col.into_simple_expr();
                                            match &f.operation {
                                                caustics::FieldOp::Equals(v) => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).eq(sea_orm::Value::from(v)));
                                                }
                                                caustics::FieldOp::NotEquals(v) => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).ne(sea_orm::Value::from(v)));
                                                }
                                                caustics::FieldOp::Contains(s) => {
                                                    let pat = format!("%{}%", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::StartsWith(s) => {
                                                    let pat = format!("{}%", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::EndsWith(s) => {
                                                    let pat = format!("%{}", s);
                                                    cond = cond.add(Expr::expr(col_expr.clone()).like(pat));
                                                }
                                                caustics::FieldOp::IsNull => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).is_null());
                                                }
                                                caustics::FieldOp::IsNotNull => {
                                                    cond = cond.add(Expr::expr(col_expr.clone()).is_not_null());
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    query = query.filter(cond);
                                }

                                if filter.distinct {
                                    query = query.distinct();
                                }

                                let total = query.count(conn).await? as i64;
                                let mut c = self._count.take().unwrap_or_default();
                                c.#count_field_ident = Some(total as i32);
                                self._count = Some(c);
                            }
                        }
                    }
                }),
                _ => None,
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
                        .ok_or_else(|| caustics::CausticsError::InvalidIncludePath { relation: filter.relation.to_string() })?;
                    let foreign_key_value = (descriptor.get_foreign_key)(self);
                    // If FK is missing on belongs_to/has_one, skip fetching gracefully
                    if foreign_key_value.is_none() && !descriptor.is_has_many {
                        return Ok(());
                    }
                    // Always resolve fetcher for the current entity module
                    let fetcher_entity_name = {
                        let type_name = std::any::type_name::<Self>();
                        let parts: Vec<&str> = type_name.rsplit("::").collect();
                        let entity_name = parts.get(1).unwrap_or(&"").to_lowercase();
                        entity_name
                    };
                    let fetcher = registry.get_fetcher(&fetcher_entity_name)
                        .ok_or_else(|| caustics::CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;
                    // Skip regular relation fetching if this is a count-only operation
                    let mut fetched_result = if filter.include_count && filter.nested_includes.is_empty() {
                        // Count-only operation: create empty result to skip set_field
                        Box::new(None::<Vec<Selected>>) as Box<dyn std::any::Any + Send>
                    } else {
                        // Regular relation fetching
                        let result = fetcher
                            .fetch_by_foreign_key_with_selection(
                                conn,
                                foreign_key_value.clone(),
                                descriptor.foreign_key_column,
                                &fetcher_entity_name,
                                filter.relation,
                                filter,
                            )
                            .await?;
                        result
                    };


                    // Populate relation counts when requested (has_many only), independent of pagination
                    if filter.include_count && descriptor.is_has_many {
                        // Use the same foreign key extractor used for fetching and wrap into DB Value
                        let foreign_key_value_any: Option<sea_orm::Value> = (descriptor.get_foreign_key)(self).map(|v| v.to_db_value());
                        match filter.relation {
                            #(#relation_count_match_arms_selected,)*
                            _ => {}
                        }
                    }

                    // Apply nested includes recursively, if any
                    if !filter.nested_includes.is_empty() {
                        match filter.relation {
                            #(
                                #relation_names_snake_lits => { #relation_nested_apply_blocks },
                            )*
                            _ => {}
                        }
                    } else {
                    }

                    // Skip set_field for count-only operations
                    if !(filter.include_count && filter.nested_includes.is_empty()) {
                        (descriptor.set_field)(self, fetched_result);
                    }
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


        // Selected holder struct with Option<T> for all scalar fields and Selected relation fields
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
        pub struct Selected {
            #(#selected_scalar_fields,)*
            #(#selected_relation_fields,)*
            pub _count: Option<Counts>,
        }

        impl Selected {
            fn new() -> Self { Default::default() }

            pub fn column_for_alias(alias: &str) -> Option<sea_query::SimpleExpr> {
                use sea_orm::IntoSimpleExpr;
                match alias {
                    #(
                        #selected_all_field_names => Some(<Entity as sea_orm::EntityTrait>::Column::#selected_all_field_idents.into_simple_expr()),
                    )*
                    _ => None,
                }
            }

            pub fn from_model(model: Model, selected_fields: &[&str]) -> Self {
                // Convert model to Selected by copying only the selected fields
                // This ensures only requested fields are populated in the Selected struct
                let mut selected = Selected::new();

                // Use a safe approach that only accesses fields that were actually fetched
                // When field selection is used, only access selected fields
                // When no field selection, access all fields safely
                if selected_fields.is_empty() {
                    // No field selection - try to populate all fields safely
                    // Use a match pattern to only access fields that exist in the model
                    #(
                        selected.#field_names = Some(model.#field_names);
                    )*
                } else {
                    // Field selection is used - only access selected fields plus defensive fields
                    let foreign_key_fields = &[#(
                        #foreign_key_fields,
                    )*];

                    // Create a set of fields that should be accessible (fetched from database)
                    let accessible_fields = {
                        let mut fields = std::collections::HashSet::new();
                        // Add selected fields
                        for field in selected_fields {
                            fields.insert(*field);
                        }
                        // Always include primary key for relation traversal
                        fields.insert(stringify!(#current_primary_key_ident));
                        // Always include foreign key fields for belongs_to relations
                        for fk_field in foreign_key_fields {
                            fields.insert(fk_field);
                        }
                        fields
                    };

                    // Only populate fields that were actually fetched from the database
                    #(
                        if accessible_fields.contains(&stringify!(#field_names)) {
                            selected.#field_names = Some(model.#field_names);
                        }
                    )*
                }

                selected
            }


            pub fn to_model_with_relations(self) -> ModelWithRelations {
                // Convert Selected to ModelWithRelations by copying all available fields
                // This creates a complete ModelWithRelations with all fields populated
                let mut model_with_relations = ModelWithRelations::default();

                // Copy scalar fields
                #(
                    if let Some(value) = self.#field_names {
                        model_with_relations.#field_names = value;
                    }
                )*

                // Copy relation fields, converting Selected types to ModelWithRelations types
                // For now, relations are not converted - they remain as None in ModelWithRelations
                // This is safe since ModelWithRelations uses Option types for all relations

                // Copy count fields
                model_with_relations._count = self._count;

                model_with_relations
            }
        }


        impl caustics::EntitySelection for Selected {
            fn fill_from_row(row: &sea_orm::QueryResult, fields: &[&str]) -> Self {
                let mut s = Selected::new();
                #(#selected_fill_stmts)*
                s
            }


            fn set_relation(&mut self, relation_name: &str, value: Box<dyn std::any::Any + Send>) {
                match relation_name {
                    #( stringify!(#relation_init_names) => { let v = value.downcast().ok().expect("relation type"); self.#relation_init_names = *v; } ),*
                    _ => {}
                }
            }

            fn get_key(&self, field_name: &str) -> Option<caustics::CausticsKey> {
                match field_name {
                    #(#get_key_match_arms,)*
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
            fn get_value_as_db_value(&self, field_name: &str) -> Option<sea_orm::Value> {
                match field_name {
                    #(
                        stringify!(#selected_field_idents_snake) => {
                            let v = self.#selected_field_idents_snake.clone();
                            match v {
                                Some(val) => Some(sea_orm::Value::from(val)),
                                None => None,
                            }
                        }
                    ),*
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
                        let entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();
                        entity_name
                    };
                    let fetcher = registry.get_fetcher(&fetcher_entity_name)
                        .ok_or_else(|| caustics::CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;

                    // If nested relations are present, we need to ensure foreign key fields are included
                    // in the selection so that nested relations can be loaded
                    let mut modified_filter = filter.clone();
                    if filter.nested_select_aliases.is_some() {
                        // Add foreign key fields to the selection for nested relation loading
                        let mut aliases = filter.nested_select_aliases.as_ref().unwrap().clone();

                        // Add all foreign key fields for the target entity (for nested relation traversal)
                        // Get the target entity's foreign key fields from the metadata registry
                        let target_entity_name = descriptor.target_entity;
                        if let Some(target_entity_metadata) = caustics::get_entity_metadata(target_entity_name) {
                            for fk_field in target_entity_metadata.foreign_key_fields {
                                if !aliases.contains(&ToString::to_string(&fk_field)) {
                                    aliases.push(ToString::to_string(&fk_field));
                                }
                            }
                        }

                        modified_filter.nested_select_aliases = Some(aliases);
                    }

                    // Skip regular relation fetching if this is a count-only operation
                    let mut fetched_result = if filter.include_count && filter.nested_includes.is_empty() {
                        // Count-only operation: create empty result to skip set_field
                        Box::new(None::<Vec<ModelWithRelations>>) as Box<dyn std::any::Any + Send>
                    } else {
                        // Regular relation fetching
                        fetcher
                    .fetch_by_foreign_key_with_selection(
                        conn,
                        foreign_key_value.clone(),
                        descriptor.foreign_key_column,
                        &fetcher_entity_name,
                        filter.relation,
                        &modified_filter,
                            )
                            .await?
                    };


                    // Populate relation counts when requested (has_many only), independent of pagination
                    if filter.include_count && descriptor.is_has_many {
                        match filter.relation {
                            #(#relation_count_match_arms,)*
                            _ => {}
                        }
                    }

                    if !filter.nested_includes.is_empty() {
                        match filter.relation {
                            #(
                                #relation_names_snake_lits => { #selected_relation_nested_apply_blocks },
                            )*
                            _ => {}
                        }
                    }

                    // Skip set_field for count-only operations
                    if !(filter.include_count && filter.nested_includes.is_empty()) {
                        (descriptor.set_field)(self, fetched_result);
                    }
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

        // Check if this is an optional relation by looking at the foreign key field
        let is_optional = match relation.kind {
            RelationKind::HasMany => false,
            RelationKind::BelongsTo => {
                if let Some(fk_field_name) = &relation.foreign_key_field {
                    if let Some(field) = fields
                        .iter()
                        .find(|f| f.ident.as_ref().expect("Field has no identifier").to_string() == *fk_field_name)
                    {
                        is_option(&field.ty)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        };

        let rel_type = match relation.kind {
            RelationKind::HasMany => quote! { Option<Vec<#target::ModelWithRelations>> },
            RelationKind::BelongsTo => {
                if is_optional {
                    // For optional relations: Option<ModelWithRelations> (fetcher returns this)
                    quote! { Option<#target::ModelWithRelations> }
                } else {
                    // For required relations: Option<ModelWithRelations>
                    quote! { Option<#target::ModelWithRelations> }
                }
            }
        };
        // Determine foreign key field and column based on relation type
        let (foreign_key_field, foreign_key_column, get_foreign_key_closure) = match relation.kind {
            RelationKind::HasMany => {
                let id_field = current_primary_key_ident.clone();
                // For HasMany relations, the foreign key column is in the target entity
                // Use the extracted foreign_key_column if available, otherwise use mapping
                let fk_column = if let Some(fk_col) = &relation.foreign_key_column {
                    // Convert PascalCase to snake_case to match database column names
                    // This is completely dynamic and works with any foreign key column name
                    fk_col.to_snake_case()
                } else {
                    // Use the relation name + "_id" pattern
                    // This is also dynamic and works with any relation name
                    format!("{}_id", relation.name.to_snake_case())
                };
                (
                    quote! { model.#id_field },
                    fk_column,
                    quote! { |model| {
                        let id_value = model.#id_field;
                        caustics::CausticsKey::from_db_value(&id_value.into())
                    } },
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
                    quote! { |model| {
                        let fk_value = model.#foreign_key_field.as_ref();
                        fk_value.and_then(|v| caustics::CausticsKey::from_db_value(&v.clone().into()))
                    } }
                } else {
                    quote! { |model| {
                        let fk_value = model.#foreign_key_field;
                        caustics::CausticsKey::from_db_value(&fk_value.into())
                    } }
                };
                (
                    quote! { model.#foreign_key_field },
                    foreign_key_field_name.to_snake_case(),
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
        let foreign_key_column =
            syn::LitStr::new(&foreign_key_column, proc_macro2::Span::call_site());

        // Get additional metadata from relation
        let fallback_table_name = relation.name.to_snake_case();
        // Use the resolved target table name from build-time metadata
        let target_table_name_expr = quote! { #fallback_table_name };
        let current_table_name = relation
            .current_table_name
            .as_ref()
            .unwrap_or_else(|| {
                panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
            });

        // Use the resolved target table name from build-time metadata
        let target_table_name_expr = quote! { #fallback_table_name };
        let current_table_name_lit =
            syn::LitStr::new(current_table_name, proc_macro2::Span::call_site());
        // Extract primary key column names dynamically using centralized utilities
        let current_primary_key_column = get_primary_key_column_name(&fields);
        let current_primary_key_column_lit =
            syn::LitStr::new(&current_primary_key_column, proc_macro2::Span::call_site());

        // For target primary key, use the relation's primary_key_field or use current entity's primary key
        let target_primary_key_column = if let Some(pk_field) = &relation.primary_key_field {
            pk_field.clone()
        } else {
            // Use the current entity's primary key column name instead of hardcoding "id"
            current_primary_key_column.clone()
        };
        let target_primary_key_column_lit =
            syn::LitStr::new(&target_primary_key_column, proc_macro2::Span::call_site());
        let is_foreign_key_nullable_lit =
            syn::LitBool::new(relation.is_nullable, proc_macro2::Span::call_site());

        let fk_field_name_lit = match relation.kind {
            RelationKind::HasMany => syn::LitStr::new(&current_primary_key_field_name, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitStr::new(
                relation.foreign_key_field.as_ref().unwrap(),
                proc_macro2::Span::call_site(),
            ),
        };
        let current_primary_key_field_name_lit =
            syn::LitStr::new(&current_primary_key_column, proc_macro2::Span::call_site());
        let is_has_many_lit = match relation.kind {
            RelationKind::HasMany => syn::LitBool::new(true, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitBool::new(false, proc_macro2::Span::call_site()),
        };

        // Generate the correct set_field implementation based on relation type
        let set_field_impl = match relation.kind {
            RelationKind::HasMany => {
                quote! {
                    let actual_type = std::any::type_name_of_val(&*value);

                    // Try to downcast as Vec<Selected> first, then fall back to Vec<ModelWithRelations>
                    let converted_value = if let Some(selected_vec) = value.downcast_ref::<Option<Vec<#target::Selected>>>() {
                        // We got Selected objects - convert to ModelWithRelations
                        if let Some(vec) = selected_vec.as_ref() {
                            Some(vec.iter().map(|selected| selected.clone().to_model_with_relations()).collect::<Vec<_>>())
                        } else {
                            None
                        }
                    } else if let Some(model_vec) = value.downcast_ref::<Option<Vec<#target::ModelWithRelations>>>() {
                        // We got ModelWithRelations objects directly
                        model_vec.clone()
                    } else {
                        panic!("Type mismatch in set_field: expected Option<Vec<{}>> or Option<Vec<{}>>, got {}",
                            stringify!(#target::Selected), stringify!(#target::ModelWithRelations), actual_type);
                    };
                    model.#rel_field = converted_value;
                }
            }
            RelationKind::BelongsTo => {
                if is_optional {
                    quote! {
                        let actual_type = std::any::type_name_of_val(&*value);

                        // Try to downcast as Option<Selected> first, then fall back to Option<ModelWithRelations>
                        let converted_value = if let Some(selected_opt) = value.downcast_ref::<Option<#target::Selected>>() {
                            // We got Selected object - convert to ModelWithRelations
                            if let Some(selected) = selected_opt.as_ref() {
                                Some(Some(selected.clone().to_model_with_relations()))
                            } else {
                                Some(None)
                            }
                        } else if let Some(model_opt) = value.downcast_ref::<Option<#target::ModelWithRelations>>() {
                            // We got ModelWithRelations object directly
                            Some(model_opt.clone())
                        } else {
                            panic!("Type mismatch in set_field: expected Option<{}> or Option<{}>, got {}",
                                stringify!(#target::Selected), stringify!(#target::ModelWithRelations), actual_type);
                        };
                        model.#rel_field = converted_value;
                    }
                } else {
                    quote! {
                        let actual_type = std::any::type_name_of_val(&*value);

                        // Try to downcast as Option<Selected> first, then fall back to Option<ModelWithRelations>
                        let converted_value = if let Some(selected_opt) = value.downcast_ref::<Option<#target::Selected>>() {
                            // We got Selected object - convert to ModelWithRelations
                            if let Some(selected) = selected_opt.as_ref() {
                                Some(selected.clone().to_model_with_relations())
                            } else {
                                None
                            }
                        } else if let Some(model_opt) = value.downcast_ref::<Option<#target::ModelWithRelations>>() {
                            // We got ModelWithRelations object directly
                            model_opt.clone()
                        } else {
                            panic!("Type mismatch in set_field: expected Option<{}> or Option<{}>, got {}",
                                stringify!(#target::Selected), stringify!(#target::ModelWithRelations), actual_type);
                        };
                        model.#rel_field = converted_value;
                    }
                }
            }
        };

        let target_entity_name_lit = if let Some(entity_name) = &relation.target_entity_name {
            quote! { Some(#entity_name) }
        } else {
            quote! { None }
        };

        quote! {
            caustics::RelationDescriptor::<ModelWithRelations> {
                name: #name,
                set_field: |model, value| {
                    #set_field_impl
                },
                get_foreign_key: #get_foreign_key_closure,
                target_entity: #target_entity,
                foreign_key_column: #foreign_key_column,
                foreign_key_field_name: #fk_field_name_lit,
                target_table_name: #target_table_name_expr,
                current_primary_key_column: #current_primary_key_column_lit,
                current_primary_key_field_name: #current_primary_key_field_name_lit,
                target_primary_key_column: #target_primary_key_column_lit,
                target_entity_name: #target_entity_name_lit,
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

        // Check if this is an optional relation
        let is_optional = match relation.kind {
            RelationKind::HasMany => false,
            RelationKind::BelongsTo => relation.is_nullable,
        };

        let rel_type = match relation.kind {
            RelationKind::HasMany => quote! { Option<Vec<#target::Selected>> },
            RelationKind::BelongsTo => quote! { Option<#target::Selected> },
        };
        let foreign_key_column = relation.foreign_key_column.as_ref().map(|s| s.to_snake_case()).unwrap_or_else(|| current_pk_column_name.clone());
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
            RelationKind::HasMany => syn::LitStr::new(&current_primary_key_field_name, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitStr::new(relation.foreign_key_field.as_ref().unwrap(), proc_macro2::Span::call_site()),
        };
        let target_table_default = relation.name.to_snake_case();
        // Use the resolved target table name from build-time metadata
        let target_table_name_expr = quote! { #target_table_default };
        let current_primary_key_field_name_lit = syn::LitStr::new(&current_primary_key_field_name, proc_macro2::Span::call_site());
        let current_primary_key_column_lit = syn::LitStr::new(&current_primary_key_column_name, proc_macro2::Span::call_site());
        let target_primary_key_column_lit = syn::LitStr::new(&relation
            .primary_key_field.clone()
            .unwrap_or_else(|| current_primary_key_field_name.clone()), proc_macro2::Span::call_site());
        let is_has_many_lit = match relation.kind {
            RelationKind::HasMany => syn::LitBool::new(true, proc_macro2::Span::call_site()),
            RelationKind::BelongsTo => syn::LitBool::new(false, proc_macro2::Span::call_site()),
        };
        let is_foreign_key_nullable_lit =
            syn::LitBool::new(relation.is_nullable, proc_macro2::Span::call_site());

        // Generate the correct set_field implementation based on relation type
        let set_field_impl = match relation.kind {
            RelationKind::HasMany => {
                quote! {
                    let actual_type = std::any::type_name_of_val(&*value);
                    let typed_value = value.downcast::<Option<Vec<#target::Selected>>>()
                        .unwrap_or_else(|_| panic!("Type mismatch in set_field: expected Option<Vec<{}>>, got {}", stringify!(#target::Selected), actual_type));
                    model.#rel_field = *typed_value;
                }
            }
            RelationKind::BelongsTo => {
                if is_optional {
                    quote! {
                        let actual_type = std::any::type_name_of_val(&*value);
                        let typed_value = value.downcast::<Option<#target::Selected>>()
                            .unwrap_or_else(|_| panic!("Type mismatch in set_field: expected Option<{}>, got {}", stringify!(#target::Selected), actual_type));
                        model.#rel_field = Some(*typed_value);
                    }
                } else {
                    quote! {
                        let actual_type = std::any::type_name_of_val(&*value);
                        let typed_value = value.downcast::<Option<#target::Selected>>()
                            .unwrap_or_else(|_| panic!("Type mismatch in set_field: expected Option<{}>, got {}", stringify!(#target::Selected), actual_type));
                        model.#rel_field = *typed_value;
                    }
                }
            }
        };

        let target_entity_name_lit = if let Some(entity_name) = &relation.target_entity_name {
            quote! { Some(#entity_name) }
        } else {
            quote! { None }
        };

        quote! {
            caustics::RelationDescriptor::<Selected> {
                name: #name,
                set_field: |model, value| {
                    #set_field_impl
                },
                get_foreign_key: |model: &Selected| {
                    // For has_many, use current id; for belongs_to, use FK field on Selected
                    let field_name = match #is_has_many_lit { true => #current_primary_key_field_name_lit, false => #fk_field_name_lit };
                    <Selected as caustics::EntitySelection>::get_key(model, field_name)
                },
                target_entity: #target_entity,
                foreign_key_column: #foreign_key_column,
                foreign_key_field_name: #fk_field_name_lit,
                target_table_name: #target_table_name_expr,
                current_primary_key_column: #current_primary_key_column_lit,
                current_primary_key_field_name: #current_primary_key_field_name_lit,
                target_primary_key_column: #target_primary_key_column_lit,
                target_entity_name: #target_entity_name_lit,
                is_foreign_key_nullable: #is_foreign_key_nullable_lit,
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

        // Defensive field fetching is handled by the existing logic in the query builders
        // The macro-generated from_model method already includes defensive fields
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

            // Add variables for registry-based conversion
            let entity_name = entity_context.registry_name();
            let foreign_key_field_name = fk_field_name;

            // Get the primary key field name from the relation definition or use dynamic detection
            let primary_key_field_name_raw = if let Some(pk) = &relation.primary_key_field {
                pk.clone()
            } else {
                // Use the current entity's primary key field name
                get_primary_key_field_name(&fields)
            };
            let primary_key_field_name = primary_key_field_name_raw.to_snake_case();
            let primary_key_pascal = primary_key_field_name_raw.chars().next().expect("Primary key field name is empty").to_uppercase().collect::<String>()
                + &primary_key_field_name_raw[1..];
            let primary_key_variant = format_ident!("{}Equals", primary_key_pascal);
            let primary_key_field_ident = format_ident!("{}", primary_key_field_name);

            // Check if this is an optional relation
            let is_optional = if let Some(field) = fields.iter().find(|f| f.ident.as_ref().unwrap().to_string() == *fk_field_name) {
                is_option(&field.ty)
            } else {
                false
            };

            if is_optional {
                quote! {
                    SetParam::#relation_name(where_param) => {
                        match where_param {
                            #target_module::UniqueWhereParam::#primary_key_variant(key) => {
                                // Extract the value from CausticsKey for database field assignment
                                let fk_value = crate::__caustics_convert_key_to_active_value_optional(#entity_name, #foreign_key_field_name, key);
                                model.#foreign_key_field = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
                            }
                            other => {
                                // Store deferred lookup instead of executing (optional FK -> wrap in Some)
                                deferred_lookups.push(caustics::DeferredLookup::new(
                                    Box::new(other.clone()),
                                    |model, value| {
                                        let model = model.downcast_mut::<ActiveModel>().unwrap();
                                        // Extract the value from CausticsKey for database field assignment
                                        let fk_value = crate::__caustics_convert_key_to_active_value_optional(#entity_name, #foreign_key_field_name, value);
                                        model.#foreign_key_field = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
                                    },
                                    |conn: & sea_orm::DatabaseConnection, param| {
                                        let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                        Box::pin(async move {
                                            let condition: sea_query::Condition = param.clone().into();
                                            let result = #target_module::Entity::find()
                                                .filter::<sea_query::Condition>(condition)
                                                .one(conn)
                                                .await?;
                                            result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
                                                caustics::CausticsError::NotFoundForCondition {
                                                    entity: stringify!(#target_module).to_string(),
                                                    condition: format!("{:?}", param),
                                                }.into()
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
                                            result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
                                                caustics::CausticsError::NotFoundForCondition {
                                                    entity: stringify!(#target_module).to_string(),
                                                    condition: format!("{:?}", param),
                                                }.into()
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
                            #target_module::UniqueWhereParam::#primary_key_variant(key) => {
                                // Extract the value from CausticsKey for database field assignment
                                let fk_value = crate::__caustics_convert_key_to_active_value(#entity_name, #foreign_key_field_name, key);
                                model.#foreign_key_field = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
                            }
                            other => {
                                // Store deferred lookup instead of executing
                                                        deferred_lookups.push(caustics::DeferredLookup::new(
                            Box::new(other.clone()),
                            |model, value| {
                                let model = model.downcast_mut::<ActiveModel>().unwrap();
                                // Extract the value from CausticsKey for database field assignment
                                let fk_value = crate::__caustics_convert_key_to_active_value(#entity_name, #foreign_key_field_name, value);
                                model.#foreign_key_field = *fk_value.downcast::<sea_orm::ActiveValue<_>>().expect("Failed to downcast to ActiveValue");
                            },
                                     |conn: & sea_orm::DatabaseConnection, param| {
                                        let param = param.downcast_ref::<#target_module::UniqueWhereParam>().unwrap().clone();
                                        Box::pin(async move {
                                            let condition: sea_query::Condition = param.clone().into();
                                            let result = #target_module::Entity::find()
                                                .filter::<sea_query::Condition>(condition)
                                                .one(conn)
                                                .await?;
                                            result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
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
                                            result.map(|entity| caustics::CausticsKey::from_db_value(&entity.#primary_key_field_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))).ok_or_else(|| {
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
                    let fk_field_name = relation
                        .foreign_key_field
                        .as_ref()
                        .expect("Foreign key field not specified");
                    if let Some(field) = fields.iter().find(|f| {
                        f.ident
                            .as_ref()
                            .expect("Field has no identifier")
                            .to_string()
                            == *fk_field_name
                    }) {
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
            let name = field.ident.as_ref().expect("Field has no identifier");
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
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            !foreign_key_fields.contains(&field_name)
        })
        .filter_map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
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
            let relation_name_lit = syn::LitStr::new(
                &relation_name.to_lowercase(),
                proc_macro2::Span::call_site(),
            );
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
                        // Handle CausticsKey format: IdEquals(Int(1)) or IdEquals(String("abc"))
                        if let Some(id_start) = param_str.find("Equals(") {
                            let after_equals = &param_str[id_start + 7..];
                            // Find the matching closing parenthesis, accounting for nested parentheses
                            let mut paren_count = 0;
                            let mut id_end = None;
                            for (i, ch) in after_equals.char_indices() {
                                match ch {
                                    '(' => paren_count += 1,
                                    ')' => {
                                        if paren_count == 0 {
                                            id_end = Some(i);
                                            break;
                                        }
                                        paren_count -= 1;
                                    }
                                    _ => {}
                                }
                            }

                            if let Some(id_end) = id_end {
                                let key_str = &after_equals[..id_end];

                                // Parse using CausticsKey for robust type handling
                                if let Ok(caustics_key) = key_str.parse::<caustics::CausticsKey>() {
                                    target_ids.push(caustics_key.to_db_value());
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
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
            syn::LitStr::new(
                &field_name,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
        })
        .collect();
    // Generate all field identifiers for column access (PascalCase for SeaORM)
    let all_field_idents: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field
                .ident
                .as_ref()
                .expect("Field has no identifier - this should not happen in valid code")
                .to_string();
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
            syn::Ident::new(
                &pascal_case,
                field
                    .ident
                    .as_ref()
                    .expect("Field has no identifier")
                    .span(),
            )
        })
        .collect();
    // Generate snake_case field idents for macro checks
    let all_field_idents_snake: Vec<syn::Ident> = fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap().clone())
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
    // No per-entity macro exports to avoid redefinition across modules

    // Generate field conversions for to_model_with_relations method
    let to_model_field_conversions = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<Option<T>> -> Option<T>
                // Unwrap the outer Option (was it fetched?) to get Option<T> (is it null?)
                quote! { #name: self.#name.expect("Field should have been fetched") }
            } else {
                // For non-nullable fields: Option<T> -> T
                // Unwrap the Option (was it fetched?) to get T
                quote! { #name: self.#name.expect("Field should have been fetched") }
            }
        })
        .collect::<Vec<_>>();

    // Generate field conversions for from_model method
    let from_model_field_conversions = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<T> -> Option<Option<T>>
                // Wrap the Model field in Some() to indicate it was "fetched"
                quote! { #name: Some(model.#name) }
            } else {
                // For non-nullable fields: T -> Option<T>
                // Wrap the Model field in Some() to indicate it was "fetched"
                quote! { #name: Some(model.#name) }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation field conversions for from_model method
    let from_model_relation_conversions = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            match relation.kind {
                RelationKind::HasMany => {
                    // Model doesn't have relation fields, so start as None
                    quote! { #name: None }
                }
                RelationKind::BelongsTo => {
                    // Model doesn't have relation fields, so start as None
                    quote! { #name: None }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate field conversions for from_model method
    let from_model_field_conversions = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<T> -> Option<Option<T>>
                // Wrap the Model field in Some() to indicate it was "fetched"
                quote! { #name: Some(model.#name) }
            } else {
                // For non-nullable fields: T -> Option<T>
                // Wrap the Model field in Some() to indicate it was "fetched"
                quote! { #name: Some(model.#name) }
            }
        })
        .collect::<Vec<_>>();

    // Generate relation field conversions for from_model method
    let from_model_relation_conversions = relations
        .iter()
        .map(|relation| {
            let name = format_ident!("{}", relation.name.to_snake_case());
            match relation.kind {
                RelationKind::HasMany => {
                    // Model doesn't have relation fields, so start as None
                    quote! { #name: None }
                }
                RelationKind::BelongsTo => {
                    // Model doesn't have relation fields, so start as None
                    quote! { #name: None }
                }
            }
        })
        .collect::<Vec<_>>();

    // Generate field conversions for to_model_with_relations method
    let to_model_field_conversions = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("Field has no identifier");
            let is_nullable = crate::common::is_option(&field.ty);

            if is_nullable {
                // For nullable fields: Option<Option<T>> -> Option<T>
                // Unwrap the first Option (selection) and keep the second (nullability)
                quote! { #name: self.#name.flatten() }
            } else {
                // For non-nullable fields: Option<T> -> T
                // Unwrap the Option and use default if None
                quote! { #name: self.#name.unwrap_or_default() }
            }
        })
        .collect::<Vec<_>>();

    // Conditionally generate select-related code only when the feature is enabled
    #[cfg(feature = "select")]
    let select_macro_code = quote! {
        // Per-entity select! macro (nightly `pub macro` path invocation support)
        // Usage: `entity::select!(field_a, field_b)`
        // Build-time name check inline with match on valid names
        // NOTE: This uses experimental `pub macro` syntax which requires nightly Rust.
        // The select feature is therefore only available on nightly.
        pub macro select($($field:ident),* $(,)?) {{
            #[allow(unused_imports)]
            macro_rules! __check_field_ident {
                #( ( #all_field_idents_snake ) => {}; )*
                ( $other:ident ) => { compile_error!(concat!("unknown field: ", stringify!($other))); };
            }
            $( __check_field_ident!($field); )*
            struct __CausticsSelectMarker;
            impl caustics::SelectionSpec for __CausticsSelectMarker {
                type Entity = Entity;
                type Data = Selected;
                fn collect_aliases(self) -> Vec<String> { vec![ $( stringify!($field).to_string() ),* ] }
                fn to_single_column_expr(self) -> sea_orm::sea_query::SimpleExpr {
                    use sea_orm::IntoSimpleExpr;
                    let aliases = self.collect_aliases();
                    if aliases.len() != 1 {
                        panic!("Aggregate functions require exactly one field, got: {:?}", aliases);
                    }
                    let field_name = &aliases[0];
                    Selected::column_for_alias(field_name).unwrap_or_else(|| panic!("Unknown field: {}", field_name))
                }
            }
            __CausticsSelectMarker
        }}
    };

    #[cfg(not(feature = "select"))]
    let select_macro_code = quote! {};

    let expanded = quote! {
        #[allow(clippy::cmp_owned)]
        #[allow(clippy::type_complexity)]
        #[allow(clippy::too_many_arguments)]
        #[allow(clippy::possible_missing_else)]
        #[allow(clippy::unnecessary_filter_map)]
        #[allow(clippy::useless_conversion)]
        #[allow(clippy::if_same_then_else)]
        #[allow(unused_imports)]
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use caustics::{SortOrder, MergeInto, FieldOp, CausticsKey};
        use caustics::{FromModel, HasManySetHandler};
        use sea_query::{Condition, Expr, SimpleExpr};
        use sea_orm::{ColumnTrait, IntoSimpleExpr, QueryFilter, QueryOrder, QuerySelect};
        use serde_json;
        use std::sync::Arc;
        use heck::ToSnakeCase;

        pub struct EntityClient<'a, C: sea_orm::ConnectionTrait> {
            conn: &'a C,
            database_backend: sea_orm::DatabaseBackend,
        }

        pub fn get_registry<'a>() -> &'a crate::CompositeEntityRegistry {
            { crate::get_registry() }
        }

        #[derive(Debug, Clone)]
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

        // Scalar field enum alias
        #[derive(Debug, Clone)]
        pub enum ScalarField {
            #(#group_by_field_variants,)*
        }


        // Helper to map snake_case field name to ScalarField variant
        pub fn scalar_field_from_str(name: &str) -> Option<ScalarField> {
            match name {
                #(
                    #all_field_names => Some(ScalarField::#group_by_field_variants),
                )*
                _ => None,
            }
        }

        // Select macro code (conditionally generated based on feature flag)
        #select_macro_code





        // Extension traits to apply select on query builders returning Selected builders
        #[cfg(feature = "select")]
        pub trait ManySelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select<S>(self, spec: S) -> caustics::SelectManyQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + caustics::ApplyNestedIncludes<C> + Send + 'static;
        }

        #[cfg(feature = "select")]
        impl<'a, C> ManySelectExt<'a, C> for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select<S>(self, spec: S) -> caustics::SelectManyQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + caustics::ApplyNestedIncludes<C> + Send + 'static,
            {
                // implementation identical to select_typed inline
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
                    pending_nulls: self.pending_nulls,
                    cursor: self.cursor,
                    is_distinct: self.is_distinct,
                    distinct_on_fields: self.distinct_on_fields,
                    distinct_on_columns: None,
                    skip_is_negative: self.skip_is_negative,
                    _phantom: std::marker::PhantomData,
                };
                let aliases = spec.collect_aliases();
                for alias in aliases {
                    if let Some(expr) = <S::Data as caustics::EntitySelection>::column_for_alias(alias.as_str()) {
                        builder = builder.push_field(expr, alias.as_str());
                        builder.requested_aliases.push(alias);
                    }
                }
                builder
            }
        }


        #[cfg(feature = "select")]
        pub trait UniqueSelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select<S>(self, spec: S) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + caustics::ApplyNestedIncludes<C> + Send + 'static;
        }

        #[cfg(feature = "select")]
        impl<'a, C> UniqueSelectExt<'a, C> for caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select<S>(self, spec: S) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + caustics::ApplyNestedIncludes<C> + Send + 'static,
            {
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
                let aliases = spec.collect_aliases();
                for alias in aliases {
                    if let Some(expr) = <S::Data as caustics::EntitySelection>::column_for_alias(alias.as_str()) {
                        builder = builder.push_field(expr, alias.as_str());
                        builder.requested_aliases.push(alias);
                    }
                }
                builder
            }
        }

        #[cfg(feature = "select")]
        pub trait FirstSelectExt<'a, C: sea_orm::ConnectionTrait> {
            fn select<S>(self, spec: S) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + Send + 'static;
        }

        #[cfg(feature = "select")]
        impl<'a, C> FirstSelectExt<'a, C> for caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
        where
            C: sea_orm::ConnectionTrait,
            ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model> + caustics::HasRelationMetadata<ModelWithRelations> + Send + 'static,
        {
            fn select<S>(self, spec: S) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, S::Data>
            where
                S: caustics::SelectionSpec<Entity = Entity>,
                S::Data: caustics::EntitySelection + caustics::HasRelationMetadata<S::Data> + Send + 'static,
            {
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
                let aliases = spec.collect_aliases();
                for alias in aliases {
                    if let Some(expr) = <S::Data as caustics::EntitySelection>::column_for_alias(alias.as_str()) {
                        builder = builder.push_field(expr, alias.as_str());
                        builder.requested_aliases.push(alias);
                    }
                }
                builder
            }
        }

        // Include parameters for relations
        #[derive(Debug, Clone)]
        pub enum IncludeParam {
            #(#include_enum_variants,)*
        }


        // Include on select builders
        #[cfg(feature = "select")]
        pub trait SelectManyIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
        }
        #[cfg(feature = "select")]
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

        #[cfg(feature = "select")]
        pub trait SelectUniqueIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectUniqueQueryBuilder<'a, C, Entity, Selected>;
        }
        #[cfg(feature = "select")]
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

        #[cfg(feature = "select")]
        pub trait SelectFirstIncludeExt<'a, C: sea_orm::ConnectionTrait> {
            fn with(self, include: IncludeParam) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>;
            fn include(self, includes: Vec<IncludeParam>) -> caustics::SelectFirstQueryBuilder<'a, C, Entity, Selected>;
        }
        #[cfg(feature = "select")]
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
            fn from(param: UniqueWhereParam) -> (sea_query::SimpleExpr, sea_orm::Value) {
                use sea_orm::IntoSimpleExpr;
                match param {
                    #(#unique_where_to_expr_value_arms),*
                }
            }
        }

        #[derive(Debug, Clone)]
        pub enum GroupByOrderByParam {
            #(#group_by_order_by_field_variants,)*
        }

        #[derive(Debug, Clone)]
        pub enum RelationOrderByParam {
            #(#relation_order_by_variants,)*
        }

        // Allow using RelationOrderByParam as an IntoOrderByExpr input
        #[allow(unreachable_code)]
        impl caustics::IntoOrderByExpr for RelationOrderByParam {
            fn into_order_by_expr(self) -> (sea_query::SimpleExpr, sea_orm::Order) {
                match self { #(#relation_order_into_expr_arms,)* }
            }
        }

        // Fluent order DSL for relation aggregates: user().order_by(user::posts::count(SortOrder::Desc))
        pub mod order_by {
            use super::RelationOrderByParam;
            use caustics::SortOrder;
            #(pub fn #relation_order_by_fn_names(order: SortOrder) -> RelationOrderByParam { RelationOrderByParam::#relation_order_by_fn_variants(order) })*
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

            fn is_has_many_create_operation(&self) -> bool {
                match self {
                    #(#has_many_create_flag_arms)*
                    #(#has_many_create_many_flag_arms)*
                    _ => false,
                }
            }

            fn exec_has_many_create_on_conn<'a>(
                &'a self,
                conn: &'a sea_orm::DatabaseConnection,
                parent_id: caustics::CausticsKey,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                let fut = async move {
                    match self {
                        #(#has_many_create_exec_conn_arms)*
                        #(#has_many_create_many_exec_conn_arms)*
                        _ => Ok(()),
                    }
                };
                Box::pin(fut)
            }

            fn exec_has_many_create_on_txn<'a>(
                &'a self,
                txn: &'a sea_orm::DatabaseTransaction,
                parent_id: caustics::CausticsKey,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
                let fut = async move {
                    match self {
                        #(#has_many_create_exec_txn_arms)*
                        #(#has_many_create_many_exec_txn_arms)*
                        _ => Ok(()),
                    }
                };
                Box::pin(fut)
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

        // Allow using typed OrderByParam directly with generic order_by()
        impl caustics::IntoOrderByExpr for OrderByParam {
            fn into_order_by_expr(self) -> (sea_query::SimpleExpr, sea_orm::Order) {
                use sea_orm::IntoSimpleExpr;
                let (col, ord): (<Entity as EntityTrait>::Column, sea_orm::Order) = self.into();
                (col.into_simple_expr(), ord)
            }
        }

        // Aggregate selection enums removed - use select! syntax instead

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
            // AggregateSelectorExt and GroupBySelectorExt removed - use select! syntax instead
            pub use super::GroupByHavingAggExt;
            pub use super::GroupByAggExt;
            pub use super::AggregateAggExt;
            #[cfg(feature = "select")]
            pub use super::ManySelectExt;
            #[cfg(feature = "select")]
            pub use super::UniqueSelectExt;
            #[cfg(feature = "select")]
            pub use super::FirstSelectExt;
            #[cfg(feature = "select")]
            pub use super::SelectManyIncludeExt;
            #[cfg(feature = "select")]
            pub use super::SelectUniqueIncludeExt;
            #[cfg(feature = "select")]
            pub use super::SelectFirstIncludeExt;
            pub use super::RelationOrderExt;
            pub use super::SelectManyRelationOrderExt;
        }

        // AggregateSelectorExt trait removed - use select! syntax instead

        // AggregateSelectorExt implementation removed - use select! syntax instead

        // GroupBySelectorExt trait removed - use select! syntax instead

        // Extend GroupByQueryBuilder with typed aggregate selectors via a local trait
        pub trait GroupByAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn sum<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn avg<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn min<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn max<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> GroupByAggExt<'a, C> for caustics::GroupByQueryBuilder<'a, C, Entity> {
            fn sum<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::sum(field.to_simple_expr())), alias)); self }
            fn avg<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::avg(field.to_simple_expr())), alias)); self }
            fn min<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::min(field.to_simple_expr())), alias)); self }
            fn max<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::max(field.to_simple_expr())), alias)); self }
        }

        // GroupByAggOrderParam enum removed - use select! syntax instead

        // Typed aggregate HAVING helpers
        pub trait GroupByHavingAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn having_sum_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_sum_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_sum_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_sum_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_sum_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_sum_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;

            fn having_avg_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_avg_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_avg_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_avg_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_avg_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_avg_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;

            fn having_min_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_min_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_min_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_min_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_min_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_min_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;

            fn having_max_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_max_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_max_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_max_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_max_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
            fn having_max_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(self, field: F, value: V) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> GroupByHavingAggExt<'a, C> for caustics::GroupByQueryBuilder<'a, C, Entity> {
            fn having_sum_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_sum_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_sum_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_sum_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_sum_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_sum_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::sum(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_avg_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_avg_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_avg_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_avg_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_avg_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_avg_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::avg(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_min_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_min_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_min_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_min_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_min_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_min_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::min(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }

            fn having_max_gt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gt(value.into()); self.having.push(cond); self }
            fn having_max_gte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).gte(value.into()); self.having.push(cond); self }
            fn having_max_lt<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lt(value.into()); self.having.push(cond); self }
            fn having_max_lte<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).lte(value.into()); self.having.push(cond); self }
            fn having_max_eq<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).eq(value.into()); self.having.push(cond); self }
            fn having_max_ne<F: caustics::FieldSelection<Entity>, V: Into<sea_orm::Value>>(mut self, field: F, value: V) -> Self { let e = sea_orm::sea_query::SimpleExpr::FunctionCall(sea_orm::sea_query::Func::max(field.to_simple_expr())); let cond = sea_orm::sea_query::Expr::expr(e).ne(value.into()); self.having.push(cond); self }
        }

        // Add typed aggregate selection for non-group aggregate queries via a trait
        pub trait AggregateAggExt<'a, C: sea_orm::ConnectionTrait> {
            fn sum<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn avg<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn min<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
            fn max<F: caustics::FieldSelection<Entity>>(self, field: F, alias: &'static str) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> AggregateAggExt<'a, C> for caustics::AggregateQueryBuilder<'a, C, Entity> {
            fn sum<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::sum(field.to_simple_expr())), alias, "sum")); self }
            fn avg<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::avg(field.to_simple_expr())), alias, "avg")); self }
            fn min<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::min(field.to_simple_expr())), alias, "min")); self }
            fn max<F: caustics::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self { self.aggregates.push((sea_query::SimpleExpr::FunctionCall(sea_query::Func::max(field.to_simple_expr())), alias, "max")); self }
        }


        #[derive(Clone, Debug)]
        pub struct Create {
            #(#required_struct_fields,)*
            #(#foreign_key_relation_fields,)*
            pub _params: Vec<SetParam>,
        }

        pub(crate) fn __extract_id(m: &<Entity as sea_orm::EntityTrait>::Model) -> caustics::CausticsKey {
            caustics::CausticsKey::from_db_value(&m.#current_primary_key_ident.into()).unwrap_or_else(|| caustics::CausticsKey::I32(0))
        }

        impl Create {
            pub(crate) fn into_active_model<C: sea_orm::ConnectionTrait>(mut self) -> (ActiveModel, Vec<caustics::DeferredLookup>, Vec<caustics::PostInsertOp<'static>>) {
                let mut model = ActiveModel::new();
                let mut deferred_lookups = Vec::new();
                let mut post_insert_ops: Vec<caustics::PostInsertOp<'static>> = Vec::new();

                // Generate UUID for UUID primary keys if not already set
                #uuid_pk_check

                #(#required_assigns)*
                #(#foreign_key_assigns)*

                // Process SetParam values
                for param in self._params {
                    match param {
                        #(#relation_connect_deferred_match_arms,)*
                        #(#relation_disconnect_match_arms,)*
                        #(#has_many_create_match_arms,)*
                        #(#has_many_create_many_match_arms,)*
                        other => {
                            // For non-relation SetParam values, use the normal merge_into
                            other.merge_into(&mut model);
                        }
                    }
                }
                (model, deferred_lookups, post_insert_ops)
            }
        }

        #model_with_relations_impl
        #relation_metadata_impl

        // Typed distinct extension for ManyQueryBuilder at module scope
        pub trait DistinctFieldsExt<'a, C: sea_orm::ConnectionTrait> {
            fn distinct(self, fields: Vec<ScalarField>) -> Self;
        }

        impl<'a, C: sea_orm::ConnectionTrait> DistinctFieldsExt<'a, C>
            for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        {
            fn distinct(mut self, fields: Vec<ScalarField>) -> Self {
                let mut exprs: Vec<SimpleExpr> = Vec::with_capacity(fields.len());
                let mut cols: Vec<<Entity as EntityTrait>::Column> = Vec::with_capacity(fields.len());
                for f in fields {
                    let e = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    let c = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants,)*
                    };
                    exprs.push(e);
                    cols.push(c);
                }
                self = self.distinct_on(exprs);
                self.distinct_on_columns(cols)
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
                let mut cols: Vec<<Entity as EntityTrait>::Column> = Vec::with_capacity(fields.len());
                for f in fields {
                    let e = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants.into_simple_expr(),)*
                    };
                    let c = match f {
                        #(ScalarField::#group_by_field_variants => <Entity as EntityTrait>::Column::#group_by_field_variants,)*
                    };
                    exprs.push(e);
                    cols.push(c);
                }
                self.distinct_on_fields = Some(exprs);
                self.distinct_on_columns = Some(cols);
                self.is_distinct = true;
                self
            }
        }

        impl<'a, C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait> EntityClient<'a, C> {
            pub fn new(conn: &'a C, database_backend: sea_orm::DatabaseBackend) -> Self {
                Self { conn, database_backend }
            }

            pub fn find_unique(&self, condition: UniqueWhereParam) -> caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let registry = get_registry();
                caustics::UniqueQueryBuilder {
                    query: <Entity as EntityTrait>::find().filter::<Condition>(condition.clone().into()),
                    conn: self.conn,
                    relations_to_fetch: vec![],
                    registry,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn find_first(&self, conditions: Vec<WhereParam>) -> caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
                let registry = get_registry();
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
                let registry = get_registry();
                let query = <Entity as EntityTrait>::find().filter::<Condition>(where_params_to_condition(conditions, self.database_backend));
                caustics::ManyQueryBuilder {
                    query,
                    conn: self.conn,
                    relations_to_fetch: vec![],
                    registry,
                    database_backend: self.database_backend,
                    reverse_order: false,
                    pending_order_bys: Vec::new(),
                    pending_nulls: None,
                    cursor: None,
                    is_distinct: false,
                    distinct_on_fields: None,
                    distinct_on_columns: None,
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

            // NOTE: Aggregation and distinct builder facades will be added incrementally



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

            // group_by_order_by_aggregates method removed - use select! syntax instead



            pub fn create(&self, #(#required_fn_args,)* #(#foreign_key_relation_args,)* _params: Vec<SetParam>) -> caustics::CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations> {
                let create = Create {
                    #(#required_inits,)*
                    #(#foreign_key_relation_inits,)*
                    _params,
                };
                let (model, deferred_lookups, post_ops) = create.into_active_model::<C>();
                caustics::CreateQueryBuilder {
                    model,
                    conn: self.conn,
                    deferred_lookups,
                    post_insert_ops: post_ops,
                    id_extractor: (__extract_id as fn(&<Entity as sea_orm::EntityTrait>::Model) -> caustics::CausticsKey),
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn create_many(&self, creates: Vec<Create>) -> caustics::CreateManyQueryBuilder<'a, C, Entity, ActiveModel>
            where
                C: sea_orm::ConnectionTrait,
            {
                let mut items = Vec::with_capacity(creates.len());
                for c in creates {
                    let (model, deferred_lookups, post_ops) = c.into_active_model::<C>();
                    items.push((
                        model,
                        deferred_lookups,
                        post_ops,
                        (__extract_id as fn(&<Entity as sea_orm::EntityTrait>::Model) -> caustics::CausticsKey),
                    ));
                }
                caustics::CreateManyQueryBuilder {
                    items,
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub fn update(&self, condition: UniqueWhereParam, changes: Vec<SetParam>) -> caustics::UnifiedUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, SetParam, crate::CompositeEntityRegistry>
            where
                C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
                ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model>
                    + caustics::HasRelationMetadata<ModelWithRelations>
                    + 'static,
            {
                let metadata_provider = get_registry();
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
                                let id_val: CausticsKey = model.#current_primary_key_ident.into();
                                Ok(id_val.to_db_value())
                            } else {
                                Err(sea_orm::DbErr::RecordNotFound("No record matched for has_many set".to_string()))
                            }
                        };
                        Box::pin(fut)
                    }
                });
                let has_many_any = changes.iter().any(|c| {
                    <SetParam as caustics::SetParamInfo>::is_has_many_set_operation(c) ||
                    <SetParam as caustics::SetParamInfo>::is_has_many_create_operation(c)
                });
                if has_many_any {
                    caustics::UnifiedUpdateQueryBuilder::Relations(caustics::HasManySetUpdateQueryBuilder {
                        condition: cond,
                        changes,
                        conn: self.conn,
                        metadata_provider: &*metadata_provider,
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

            pub fn update_many(&self, conditions: Vec<WhereParam>, changes: Vec<SetParam>) -> caustics::UpdateManyQueryBuilder<'a, C, Entity, ActiveModel, SetParam>
            where
                C: sea_orm::ConnectionTrait,
            {
                let cond = where_params_to_condition(conditions, self.database_backend);
                caustics::UpdateManyQueryBuilder {
                    condition: cond,
                    changes,
                    conn: self.conn,
                    _phantom: std::marker::PhantomData,
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
                let (model, deferred_lookups, post_insert_ops) = create.into_active_model::<C>();
                caustics::UpsertQueryBuilder {
                    condition: condition.into(),
                    create: (
                        model,
                        deferred_lookups,
                        post_insert_ops,
                        (__extract_id as fn(&<Entity as sea_orm::EntityTrait>::Model) -> caustics::CausticsKey),
                    ),
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
                foreign_key_value: Option<caustics::CausticsKey>,
                foreign_key_column: &'a str,
                target_entity: &'a str,
                relation_name: &'a str,
                filter: &'a caustics::RelationFilter,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>, sea_orm::DbErr>> + Send + 'a>> {
                Box::pin(async move {
                    match relation_name {
                        #(
                        #relation_names_snake_lits => {
                            #relation_fetcher_bodies
                        }
                        )*
                        _ => {
                            Err(caustics::CausticsError::RelationNotFound { relation: relation_name.to_string() }.into())
                        }
                    }
                })
            }

            fn fetch_by_foreign_key_with_selection<'a>(
                &'a self,
                conn: &'a C,
                foreign_key_value: Option<caustics::CausticsKey>,
                foreign_key_column: &'a str,
                target_entity: &'a str,
                relation_name: &'a str,
                filter: &'a caustics::RelationFilter,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>, sea_orm::DbErr>> + Send + 'a>> {
                Box::pin(async move {
                    match relation_name {
                        #(
                        #relation_names_snake_lits => {
                            #relation_fetcher_bodies_selected
                        }
                        )*
                        _ => {
                            Err(caustics::CausticsError::RelationNotFound { relation: relation_name.to_string() }.into())
                        },
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

        // Relation aggregate orderBy (order parents by child counts)
        pub trait RelationOrderExt<'a, C: sea_orm::ConnectionTrait> {
            fn order_by(self, order: RelationOrderByParam) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>;
            fn order_by_many(self, order: Vec<RelationOrderByParam>) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>;
        }

        impl<'a, C: sea_orm::ConnectionTrait> RelationOrderExt<'a, C>
            for caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
        {
            #[allow(unreachable_code)]
            fn order_by(mut self, order: RelationOrderByParam) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
                match order { #(#relation_order_match_arms_many)* }
                self
            }
            #[allow(unreachable_code)]
            fn order_by_many(mut self, order: Vec<RelationOrderByParam>) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> { for o in order { match o { #(#relation_order_match_arms_many)* } } self }
        }

        pub trait SelectManyRelationOrderExt<'a, C: sea_orm::ConnectionTrait> {
            fn order_by(self, order: RelationOrderByParam) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
            fn order_by_many(self, order: Vec<RelationOrderByParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>;
        }

        impl<'a, C: sea_orm::ConnectionTrait> SelectManyRelationOrderExt<'a, C>
            for caustics::SelectManyQueryBuilder<'a, C, Entity, Selected>
        {
            #[allow(unreachable_code)]
            fn order_by(mut self, order: RelationOrderByParam) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected> {
                match order { #(#relation_order_match_arms_select_many)* }
                self
            }
            #[allow(unreachable_code)]
            fn order_by_many(mut self, order: Vec<RelationOrderByParam>) -> caustics::SelectManyQueryBuilder<'a, C, Entity, Selected> { for o in order { match o { #(#relation_order_match_arms_select_many)* } } self }
        }
    };

    Ok(expanded)
}
