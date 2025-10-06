use crate::common::is_option;
use crate::entity::types::{Relation, RelationKind};
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

#[allow(clippy::cmp_owned)]
pub fn generate_relation_submodules(relations: &[Relation], fields: &[&syn::Field]) -> TokenStream {
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
        let create_variant = format_ident!("Create{}", relation.name.to_pascal_case());
        let create_many_variant = format_ident!("CreateMany{}", relation.name.to_pascal_case());
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

        // Generate create helpers only for has_many
        let create_fns = if matches!(relation.kind, RelationKind::HasMany) {
            quote! {
                pub fn create(items: Vec<super::#target::Create>) -> super::SetParam {
                    super::SetParam::#create_variant(items)
                }
                pub fn create_many(items: Vec<super::#target::Create>) -> super::SetParam {
                    super::SetParam::#create_many_variant(items)
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
        let foreign_key_column_ident = match &relation.foreign_key_column {
            Some(fk_col) => format_ident!("{}", fk_col.to_pascal_case()),
            None => {
                panic!("No foreign key column specified for relation '{}'.\n\nPlease add 'to' attribute with target column.\n\nExample:\n    #[sea_orm(\n        has_many = \"super::post::Entity\",\n        from = \"Column::UserId\",\n        to = \"super::post::Column::AuthorId\"\n    )]\n    posts: Vec<Post>,", relation.name)
            }
        };

        let order_by_relation_fn = if matches!(relation.kind, RelationKind::HasMany) {
            let variant_ident = format_ident!("{}Count", relation_name.to_pascal_case());
            quote! {
                pub fn order_by(order: caustics::SortOrder) -> super::RelationOrderByParam {
                    super::RelationOrderByParam::#variant_ident(order)
                }
            }
        } else {
            quote! {}
        };

        let variant = if matches!(relation.kind, RelationKind::HasMany) {
            format_ident!("{}Count", relation_name.to_pascal_case())
        } else {
            format_ident!("{}Field", relation_name.to_pascal_case())
        };

        // Generate field ordering functions for relation fields
        let relation_field_order_fns = if matches!(relation.kind, RelationKind::BelongsTo) {
            // For belongs_to relations, we can order by fields in the related entity
            let field_variant = format_ident!("{}Field", relation_name.to_pascal_case());
            quote! {
                // Field ordering for belongs_to relations
                // This allows ordering by fields in the related entity
                // Example: post::user::name::order(SortOrder::Asc)
                pub fn field(field_name: &str, order: caustics::SortOrder) -> super::RelationOrderByParam {
                    super::RelationOrderByParam::#field_variant(field_name.to_string(), order)
                }
            }
        } else {
            quote! {}
        };

        // Generate count function for has_many relations
        let count_fn = if matches!(relation.kind, RelationKind::HasMany) {
            let variant_ident = format_ident!("{}Count", relation_name.to_pascal_case());
            quote! {
                // Count function for relation ordering
                pub fn count(order: caustics::SortOrder) -> super::RelationOrderByParam {
                    super::RelationOrderByParam::#variant_ident(order)
                }
            }
        } else {
            quote! {}
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
                        distinct: false,
                    }
                }


                // Helper to convert typed WhereParams into generic Filters
                pub fn filters_from_where(params: Vec<super::#target::WhereParam>) -> Vec<super::Filter> {
                    super::#target::where_params_to_filters(params)
                }

                // Closure: include(|rel| rel.filter(...).order_by(...).take(...).skip(...).with(...).select(...).count())
                pub fn include<F>(f: F) -> super::RelationFilter
                where
                    F: FnOnce(RelBuilder) -> RelBuilder,
                {
                    let core = caustics::IncludeBuilderCore::new();
                    let b = f(RelBuilder { core });
                    let mut g = b.core.build(#relation_name_lit);

                    // If there are field selections AND nested includes, automatically include foreign key fields
                    if let Some(ref mut select_aliases) = g.nested_select_aliases {
                        if !g.nested_includes.is_empty() && !select_aliases.is_empty() {
                            // Get entity metadata to find foreign key fields for nested relations
                            let target_entity_name = stringify!(#target);
                            // Extract just the last part of the path (e.g., "super::course" -> "course")
                            let target_module_name = target_entity_name.split("::").last().unwrap_or(target_entity_name).trim();
                            // Convert snake_case to PascalCase
                            let entity_name = target_module_name.split('_').map(|s| {
                                let mut chars = s.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => first.to_uppercase().chain(chars).collect(),
                                }
                            }).collect::<String>();

                            // Use metadata system when available
                            // For now, skip automatic foreign key inclusion
                        }
                    }

                    super::RelationFilter {
                        relation: g.relation,
                        filters: g.filters,
                        nested_select_aliases: g.nested_select_aliases,
                        nested_includes: g.nested_includes,
                        take: g.take,
                        skip: g.skip,
                        order_by: g.order_by,
                        cursor_id: g.cursor_id,
                        include_count: g.include_count,
                        distinct: g.distinct,
                    }
                }

                pub struct RelBuilder { core: caustics::IncludeBuilderCore }
                impl RelBuilder {
                    // For relation includes, keep field-based order API
                    pub fn order_by(mut self, params: Vec<super::#target::OrderByParam>) -> Self {
                        let mut pairs = Vec::new();
                        for p in params.into_iter() {
                            let (col, ord): (<super::#target::Entity as EntityTrait>::Column, sea_orm::Order) = p.into();
                            let name = format!("{:?}", col).to_string().to_snake_case();
                            pairs.push((name, match ord { sea_orm::Order::Asc => caustics::SortOrder::Asc, _ => caustics::SortOrder::Desc }));
                        }
                        self.core.push_order_pairs(pairs);
                        self
                    }
                    pub fn filter(mut self, filters: Vec<super::#target::WhereParam>) -> Self {
                        let converted = super::#target::where_params_to_filters(filters);
                        self.core.push_filters(converted);
                        self
                    }

                    pub fn take(mut self, n: i64) -> Self { self.core.set_take(n); self }
                    pub fn skip(mut self, n: i64) -> Self { self.core.set_skip(n); self }
                    pub fn cursor(mut self, id: impl Into<caustics::CausticsKey>) -> Self { self.core.set_cursor_id(id.into()); self }
                    pub fn with(mut self, include: super::#target::RelationFilter) -> Self {
                        self.core.with_nested(include.into());
                        self
                    }
                    pub fn select(mut self, spec: impl caustics::SelectionSpec<Entity = super::#target::Entity, Data = super::#target::Selected>) -> Self {
                        let aliases = spec.collect_aliases();
                        self.core.set_select_aliases(aliases);
                        self
                    }
                    pub fn count(mut self) -> Self { self.core.enable_count(); self }
                    pub fn distinct(mut self) -> Self { self.core.enable_distinct(); self }
                }

                // (Typed where/order/take/skip helpers are intentionally omitted to avoid cross-module privacy issues.)
                // (Nested reads args builder intentionally omitted for now.)

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
                        distinct: false,
                    }
                }


                pub fn with(include: super::#target::RelationFilter) -> super::RelationFilter {
                    fetch_with_includes(vec![include])
                }

                // with_many was redundant; use multiple `.with(...)` calls or nested include(|rel| ...)




                // Relation-aggregate orderBy sugar entry: relation::order_by(child_field::count(order))
                #order_by_relation_fn

                // Count function for relation ordering
                #count_fn

                // Field ordering functions for relation fields
                #relation_field_order_fns

                pub fn connect(where_param: super::#target::UniqueWhereParam) -> super::SetParam {
                    super::SetParam::#connect_variant(where_param)
                }

                #set_fn
                #create_fns
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
