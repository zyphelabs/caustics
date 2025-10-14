use crate::types::ApplyNestedIncludes;
use crate::types::EntityRegistry;
use crate::types::SelectionSpec;
use crate::types::{IntoOrderSpec, NullsOrder};
use crate::EntitySelection;
use crate::{FromModel, HasRelationMetadata, RelationFilter};
use sea_orm::sea_query::{Condition, Expr, SimpleExpr};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select,
};

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub database_backend: DatabaseBackend,
    pub reverse_order: bool,
    pub pending_order_bys: Vec<(SimpleExpr, sea_orm::Order)>,
    pub pending_nulls: Option<NullsOrder>,
    pub cursor: Option<Vec<(SimpleExpr, sea_orm::Value)>>,
    pub is_distinct: bool,
    pub distinct_on_fields: Option<Vec<SimpleExpr>>,
    pub distinct_on_columns: Option<Vec<<Entity as EntityTrait>::Column>>,
    pub skip_is_negative: bool,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations: FromModel<Entity::Model>
        + HasRelationMetadata<ModelWithRelations>
        + crate::types::ApplyNestedIncludes<C>
        + Send
        + 'static,
{
    pub fn select<S>(
        self,
        spec: S,
    ) -> crate::query_builders::select_many::SelectManyQueryBuilder<'a, C, Entity, S::Data>
    where
        S: SelectionSpec<Entity = Entity>,
        S::Data: EntitySelection
            + HasRelationMetadata<S::Data>
            + crate::types::ApplyNestedIncludes<C>
            + Send
            + 'static,
    {
        let mut builder = crate::query_builders::select_many::SelectManyQueryBuilder {
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
            distinct_on_columns: self.distinct_on_columns,
            skip_is_negative: self.skip_is_negative,
            _phantom: std::marker::PhantomData,
        };
        let aliases = spec.collect_aliases();
        for alias in aliases {
            if let Some(expr) = <S::Data as EntitySelection>::column_for_alias(alias.as_str()) {
                builder = builder.push_field(expr, alias.as_str());
                builder.requested_aliases.push(alias);
            }
        }
        builder
    }
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
        if offset < 0 {
            // Defer error until exec to maintain builder signature
            self.skip_is_negative = true;
        } else {
            self.query = self.query.offset(offset as u64);
        }
        self
    }

    /// Order the results (supports scalar columns or relation aggregates via IntoOrderByExpr)
    pub fn order_by<T>(mut self, order_spec: T) -> Self
    where
        T: IntoOrderSpec,
    {
        let (expr, order, nulls) = order_spec.into_order_spec();
        self.pending_order_bys.push((expr, order));
        if nulls.is_some() {
            self.pending_nulls = nulls;
        }
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

    /// Distinct on typed columns (enables native DISTINCT ON on Postgres)
    pub fn distinct_on_columns(mut self, cols: Vec<<Entity as EntityTrait>::Column>) -> Self {
        self.distinct_on_columns = Some(cols);
        self.query = self.query.distinct();
        self.is_distinct = true;
        self
    }

    /// Internal helper used by generated code to provide a cursor column/value
    pub fn with_cursor(mut self, expr: SimpleExpr, value: sea_orm::Value) -> Self {
        match &mut self.cursor {
            Some(parts) => parts.push((expr, value)),
            None => self.cursor = Some(vec![(expr, value)]),
        }
        self
    }

    /// Execute the query and return multiple results
    pub async fn exec(self) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        if self.skip_is_negative {
            return Err(crate::types::CausticsError::QueryValidation {
                message: "skip must be >= 0".to_string(),
            }
            .into());
        }
        let mut query = self.query.clone();
        // Apply cursor filtering if provided
        if let Some(cursor_parts) = &self.cursor {
            // Determine effective order to derive comparison operator
            let first_order = self
                .pending_order_bys
                .first()
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

            // Exclusive comparator for proper cursor pagination (cursor row excluded)
            // For composite cursors, apply lexicographic comparison over all parts
            // WHERE (a > ca) OR (a = ca AND b > cb) OR (a = ca AND b = cb AND c > cc) ...
            if !cursor_parts.is_empty() {
                let mut disjunction = Condition::any();
                for i in 0..cursor_parts.len() {
                    let mut conjunction = Condition::all();
                    // Prefix equals on earlier parts
                    for j in 0..i {
                        let (expr_eq, val_eq) = &cursor_parts[j];
                        conjunction = conjunction.add(Expr::expr(expr_eq.clone()).eq(val_eq.clone()));
                    }
                    // Strict comparator on current part
                    let (expr_cmp, val_cmp) = &cursor_parts[i];
                    let cmp = match effective_order {
                        sea_orm::Order::Asc => Expr::expr(expr_cmp.clone()).gt(val_cmp.clone()),
                        sea_orm::Order::Desc => Expr::expr(expr_cmp.clone()).lt(val_cmp.clone()),
                        _ => Expr::expr(expr_cmp.clone()).gt(val_cmp.clone()),
                    };
                    conjunction = conjunction.add(cmp);
                    disjunction = disjunction.add(conjunction);
                }
                query = query.filter(disjunction);
            }

            // If no explicit order_by was provided, order by the cursor column for stability
            if self.pending_order_bys.is_empty() {
                let ord = if self.reverse_order { sea_orm::Order::Desc } else { sea_orm::Order::Asc };
                // Order by all cursor parts to preserve lexicographic ordering
                for (expr, _) in cursor_parts.iter() {
                    query = query.order_by(expr.clone(), ord.clone());
                }
            }
        }
        // Apply any pending orderings here, so reversal is respected regardless of call order
        if !self.pending_order_bys.is_empty() {
            // Apply NULLS ordering for the primary order expression if requested
            if let Some(n) = self.pending_nulls {
                if let Some((first_expr, _)) = self.pending_order_bys.first() {
                    let nulls_expr = Expr::expr(first_expr.clone()).is_null();
                    match n {
                        NullsOrder::First => {
                            query = query.order_by(nulls_expr, sea_orm::Order::Desc);
                        }
                        NullsOrder::Last => {
                            query = query.order_by(nulls_expr, sea_orm::Order::Asc);
                        }
                    }
                }
            }

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

        // Apply per-field distinct if provided:
        // - Postgres: try to use DISTINCT ON natively when available
        // - Others: best-effort emulation via GROUP BY
        if let Some(fields) = &self.distinct_on_fields {
            if !fields.is_empty() {
                match self.database_backend {
                    DatabaseBackend::Postgres => {
                        if let Some(cols) = &self.distinct_on_columns {
                            sea_orm::QueryTrait::query(&mut query).distinct_on(cols.clone());
                        } else {
                            for f in fields {
                                sea_orm::QueryTrait::query(&mut query)
                                    .add_group_by(std::iter::once(f.clone()));
                            }
                        }
                    }
                    _ => {
                        for f in fields {
                            sea_orm::QueryTrait::query(&mut query)
                                .add_group_by(std::iter::once(f.clone()));
                        }
                    }
                }
            }
        }

        // Emit before hook
        let entity_name = core::any::type_name::<Entity>();
        crate::hooks::emit_before(&crate::hooks::QueryEvent {
            builder: "ManyQueryBuilder",
            entity: entity_name,
            details: crate::hooks::compose_details("select_many", entity_name),
        });
        let start = std::time::Instant::now();
        let res = if self.relations_to_fetch.is_empty() {
            query.all(self.conn).await.map(|models| {
                models
                    .into_iter()
                    .map(|model| ModelWithRelations::from_model(model))
                    .collect()
            })
        } else {
            self.exec_with_relations_with_query(query).await
        };
        // Emit after hook
        match &res {
            Ok(rows) => crate::hooks::emit_after(
                &crate::hooks::QueryEvent {
                    builder: "ManyQueryBuilder",
                    entity: entity_name,
                    details: crate::hooks::compose_details("select_many", entity_name),
                },
                &crate::hooks::QueryResultMeta {
                    row_count: Some(rows.len()),
                    error: None,
                    elapsed_ms: Some(start.elapsed().as_millis()),
                },
            ),
            Err(e) => crate::hooks::emit_after(
                &crate::hooks::QueryEvent {
                    builder: "ManyQueryBuilder",
                    entity: entity_name,
                    details: crate::hooks::compose_details("select_many", entity_name),
                },
                &crate::hooks::QueryResultMeta {
                    row_count: None,
                    error: Some(e.to_string()),
                    elapsed_ms: Some(start.elapsed().as_millis()),
                },
            ),
        }
        res
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations_with_query(
        self,
        query: Select<Entity>,
    ) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
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
                ApplyNestedIncludes::apply_relation_filter(
                    &mut model_with_relations,
                    conn,
                    relation_filter,
                    registry,
                )
                .await?;
            }
            models_with_relations.push(model_with_relations);
        }

        Ok(models_with_relations)
    }
}
