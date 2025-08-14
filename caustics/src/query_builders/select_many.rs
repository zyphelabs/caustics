use crate::{EntitySelection, HasRelationMetadata, RelationFilter};
use crate::types::{EntityRegistry, ApplyNestedIncludes};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select};
use sea_orm::sea_query::{Condition, Expr, SimpleExpr};

/// Query builder for selected scalar fields on many
pub struct SelectManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, Selected>
where
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub selected_fields: Vec<(SimpleExpr, String)>,
    pub requested_aliases: Vec<String>,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub database_backend: DatabaseBackend,
    pub reverse_order: bool,
    pub pending_order_bys: Vec<(SimpleExpr, sea_orm::Order)>,
    pub cursor: Option<(SimpleExpr, sea_orm::Value)>,
    pub is_distinct: bool,
    pub distinct_on_fields: Option<Vec<SimpleExpr>>,
    pub _phantom: std::marker::PhantomData<Selected>,
}

impl<'a, C, Entity, Selected> SelectManyQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + ApplyNestedIncludes<C> + Send + 'static,
{
    /// Add a scalar field (expr, alias)
    pub fn push_field(mut self, expr: SimpleExpr, alias: &str) -> Self {
        self.selected_fields.push((expr, alias.to_string()));
        self
    }

    /// Execute and return selected rows
    pub async fn exec(self) -> Result<Vec<Selected>, sea_orm::DbErr> {
        let mut query = self.query.clone();

        // Apply cursor filtering if provided (copied from ManyQueryBuilder)
        if let Some((cursor_expr, cursor_value)) = &self.cursor {
            let first_order = self
                .pending_order_bys
                .get(0)
                .map(|(_, ord)| ord.clone())
                .unwrap_or(sea_orm::Order::Asc);
            let effective_order = if self.reverse_order {
                match first_order {
                    sea_orm::Order::Asc => sea_orm::Order::Desc,
                    sea_orm::Order::Desc => sea_orm::Order::Asc,
                    other => other,
                }
            } else {
                first_order
            };
            let cmp_expr = match effective_order {
                sea_orm::Order::Asc => Expr::expr(cursor_expr.clone()).gt(cursor_value.clone()),
                sea_orm::Order::Desc => Expr::expr(cursor_expr.clone()).lt(cursor_value.clone()),
                _ => Expr::expr(cursor_expr.clone()).gt(cursor_value.clone()),
            };
            query = query.filter(Condition::all().add(cmp_expr));
        }

        // Apply orderings
        if !self.pending_order_bys.is_empty() {
            for (expr, order) in &self.pending_order_bys {
                let effective = if self.reverse_order {
                    match order {
                        sea_orm::Order::Asc => sea_orm::Order::Desc,
                        sea_orm::Order::Desc => sea_orm::Order::Asc,
                        other => other.clone(),
                    }
                } else {
                    order.clone()
                };
                query = query.order_by(expr.clone(), effective);
            }
        }

        // Emulate distinct on by group-by when present
        if let Some(fields) = &self.distinct_on_fields {
            if !fields.is_empty() {
                for f in fields {
                    sea_orm::QueryTrait::query(&mut query).add_group_by(std::iter::once(f.clone()));
                }
            }
        }

        // Ensure required key columns for any requested relations are added implicitly by resolving alias to expr via Selected
        let mut selected = self.selected_fields.clone();
        if !self.relations_to_fetch.is_empty() {
            for rf in &self.relations_to_fetch {
                if let Some(desc) = Selected::get_relation_descriptor(rf.relation) {
                    let needed_alias = if desc.is_has_many { desc.current_primary_key_field_name } else { desc.foreign_key_field_name };
                    if !self.requested_aliases.iter().any(|a| a == needed_alias) {
                        if let Some(expr) = Selected::column_for_alias(needed_alias) {
                            selected.push((expr, needed_alias.to_string()));
                        }
                    }
                }
            }
        }

        let mut select = query.select_only();
        for (expr, alias) in &selected {
            select.expr_as(expr.clone(), alias.as_str());
        }

        let stmt = select.build(self.database_backend);
        let rows = self.conn.query_all(stmt).await?;
        let mut out: Vec<Selected> = Vec::with_capacity(rows.len());
        let field_names: Vec<&str> = self.requested_aliases.iter().map(|a| a.as_str()).collect();

        for row in rows.into_iter() {
            let mut s = Selected::fill_from_row(&row, &field_names);

            // include relations: only works if needed keys are in selection
            if !self.relations_to_fetch.is_empty() {
                for rel in Selected::relation_descriptors() {
                    let needed_key = if rel.is_has_many {
                        rel.current_primary_key_field_name
                    } else {
                        rel.foreign_key_field_name
                    };
                    // If the key wasn't selected, skip filling that relation
                    if let Some(_) = s.get_i32(needed_key) {
                        // okay
                    } else {
                        continue;
                    }
                }

                for rf in &self.relations_to_fetch {
                    <Selected as ApplyNestedIncludes<C>>::apply_relation_filter(&mut s, self.conn, rf, self.registry).await?;
                }
            }

            // clear any unselected scalar fields
            s.clear_unselected(&field_names);
            out.push(s);
        }
        Ok(out)
    }

    /// Add a relation to fetch with the selection
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }
}


