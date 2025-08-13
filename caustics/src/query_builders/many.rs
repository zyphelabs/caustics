use crate::{FromModel, HasRelationMetadata, RelationFilter};
use crate::types::{EntityRegistry, Filter, CausticsError};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select};
use sea_orm::sea_query::{Condition, Expr, SimpleExpr};

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: DatabaseBackend,
    pub reverse_order: bool,
    pub pending_order_bys: Vec<(SimpleExpr, sea_orm::Order)>,
    pub cursor: Option<(SimpleExpr, sea_orm::Value)>,
    pub is_distinct: bool,
    pub distinct_on_fields: Option<Vec<SimpleExpr>>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Limit the number of results (aligned with Prisma's i64 API)
    pub fn take(mut self, limit: i64) -> Self {
        let limit_u = if limit < 0 {
            self.reverse_order = true;
            (-limit) as u64
        } else {
            limit as u64
        };
        self.query = self.query.limit(limit_u);
        self
    }

    /// Skip a number of results (for pagination, aligned with Prisma's i64 API)
    pub fn skip(mut self, offset: i64) -> Self {
        let offset_u = if offset < 0 { 0 } else { offset as u64 };
        self.query = self.query.offset(offset_u);
        self
    }

    /// Order the results by a column
    pub fn order_by<Col>(mut self, col_and_order: impl Into<(Col, sea_orm::Order)>) -> Self
    where
        Col: sea_orm::ColumnTrait + sea_orm::IntoSimpleExpr,
    {
        let (col, order) = col_and_order.into();
        let expr = col.into_simple_expr();
        self.pending_order_bys.push((expr, order));
        self
    }

    /// Return distinct rows (across all selected columns)
    pub fn distinct_all(mut self) -> Self {
        self.query = self.query.distinct();
        self.is_distinct = true;
        self
    }

    /// EXPERIMENTAL: Distinct by specific fields (best-effort across backends)
    pub fn distinct_on(mut self, fields: Vec<SimpleExpr>) -> Self {
        self.distinct_on_fields = Some(fields);
        // Fallback behavior: apply simple DISTINCT when backend doesn't support DISTINCT ON
        self.query = self.query.distinct();
        self.is_distinct = true;
        self
    }

    /// Internal helper used by generated code to provide a cursor column/value
    pub fn with_cursor(mut self, expr: SimpleExpr, value: sea_orm::Value) -> Self {
        self.cursor = Some((expr, value));
        self
    }


    /// Execute the query and return multiple results
    pub async fn exec(self) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let mut query = self.query.clone();
        // Apply cursor filtering if provided
        if let Some((cursor_expr, cursor_value)) = &self.cursor {
            // Determine effective order to derive comparison operator
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
                // Fallback behaves like Asc
                _ => Expr::expr(cursor_expr.clone()).gt(cursor_value.clone()),
            };

            query = query.filter(Condition::all().add(cmp_expr));
        }
        // Apply any pending orderings here, so reversal is respected regardless of call order
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

        // Apply per-field distinct if provided (best-effort): emulate by grouping when possible
        if let Some(fields) = &self.distinct_on_fields {
            if !fields.is_empty() {
                for f in fields {
                    sea_orm::QueryTrait::query(&mut query).add_group_by(std::iter::once(f.clone()));
                }
            }
        }

        if self.relations_to_fetch.is_empty() {
            query.all(self.conn).await.map(|models| {
                models
                    .into_iter()
                    .map(|model| ModelWithRelations::from_model(model))
                    .collect()
            })
        } else {
            self.exec_with_relations_with_query(query).await
        }
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations_with_query(self, query: Select<Entity>) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let Self {
            
            conn,
            relations_to_fetch,
            registry,
            ..
        } = self;
        let main_results = query.all(conn).await?;

        let mut models_with_relations = Vec::new();

        for main_model in main_results {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            for relation_filter in &relations_to_fetch {
                Self::fetch_relation_for_model(
                    conn,
                    &mut model_with_relations,
                    relation_filter.relation,
                    &relation_filter.filters,
                    registry,
                )
                .await?;
            }
            models_with_relations.push(model_with_relations);
        }

        Ok(models_with_relations)
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relation_name: &str,
        _filters: &[Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Use the actual relation fetcher implementation
        let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
            .ok_or_else(|| CausticsError::RelationNotFound { relation: relation_name.to_string() })?;

        // Get the foreign key value from the model
        let foreign_key_value = (descriptor.get_foreign_key)(model_with_relations);

        // Use typed target entity name from the descriptor
        let extracted_entity_name = descriptor.target_entity.to_string();

        // Get the foreign key column name from the descriptor
        let foreign_key_column = descriptor.foreign_key_column;

        // Determine which entity's fetcher to use
        let is_has_many = foreign_key_column == "id";
        let fetcher_entity_name = if is_has_many {
            // Use the registry key for the current entity
            let type_name = std::any::type_name::<ModelWithRelations>();
            type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase()
        } else {
            extracted_entity_name.clone()
        };
        let fetcher = registry.get_fetcher(&fetcher_entity_name)
            .ok_or_else(|| CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;

        // Fetch the relation data
        let fetched_result = fetcher
            .fetch_by_foreign_key(
                conn,
                foreign_key_value,
                foreign_key_column,
                &fetcher_entity_name,
                relation_name,
            )
            .await?;

        // The fetcher already returns the correct type, just pass it directly
        (descriptor.set_field)(model_with_relations, fetched_result);

        Ok(())
    }
}

