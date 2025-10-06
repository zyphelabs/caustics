use crate::types::SelectionSpec;
use crate::types::{ApplyNestedIncludes, EntityRegistry, IntoOrderSpec, NullsOrder};
use crate::{EntitySelection, HasRelationMetadata, RelationFilter};
use sea_orm::sea_query::{Condition, Expr, SimpleExpr};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, Select,
};

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
    pub pending_nulls: Option<NullsOrder>,
    pub cursor: Option<(SimpleExpr, sea_orm::Value)>,
    pub is_distinct: bool,
    pub distinct_on_fields: Option<Vec<SimpleExpr>>,
    pub distinct_on_columns: Option<Vec<<Entity as EntityTrait>::Column>>,
    pub skip_is_negative: bool,
    pub _phantom: std::marker::PhantomData<Selected>,
}

impl<'a, C, Entity, Selected> SelectManyQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected:
        EntitySelection + HasRelationMetadata<Selected> + ApplyNestedIncludes<C> + Send + 'static,
{
    /// Add a scalar field (expr, alias)
    pub fn push_field(mut self, expr: SimpleExpr, alias: &str) -> Self {
        self.selected_fields.push((expr, alias.to_string()));
        self
    }

    /// Order the selection (supports scalar columns or relation aggregates via IntoOrderByExpr)
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


    /// Execute and return selected rows with type inference
    pub async fn exec<T>(self) -> Result<Vec<T>, sea_orm::DbErr>
    where
        T: From<Selected>,
    {
        let results = self.exec_internal().await?;
        Ok(results.into_iter().map(T::from).collect())
    }

    /// Internal implementation for exec
    async fn exec_internal(self) -> Result<Vec<Selected>, sea_orm::DbErr> {
        if self.skip_is_negative {
            return Err(crate::types::CausticsError::QueryValidation {
                message: "skip must be >= 0".to_string(),
            }
            .into());
        }
        let mut query = self.query.clone();

        // Apply cursor filtering if provided (copied from ManyQueryBuilder)
        if let Some((cursor_expr, cursor_value)) = &self.cursor {
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
            let cmp_expr = match effective_order {
                sea_orm::Order::Asc => Expr::expr(cursor_expr.clone()).gte(cursor_value.clone()),
                sea_orm::Order::Desc => Expr::expr(cursor_expr.clone()).lte(cursor_value.clone()),
                _ => Expr::expr(cursor_expr.clone()).gte(cursor_value.clone()),
            };
            query = query.filter(Condition::all().add(cmp_expr));
            if self.pending_order_bys.is_empty() {
                let ord = if self.reverse_order {
                    sea_orm::Order::Desc
                } else {
                    sea_orm::Order::Asc
                };
                query = query.order_by(cursor_expr.clone(), ord);
            }
        }

        // Apply orderings
        if !self.pending_order_bys.is_empty() {
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

        // Apply per-field distinct:
        // - Postgres: use native DISTINCT ON if available
        // - Others: emulate via GROUP BY
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

        // Ensure required key columns for any requested relations are added implicitly by resolving alias to expr via Selected
        let mut selected = self.selected_fields.clone();
        let mut defensive_fields = Vec::new();

        if !self.relations_to_fetch.is_empty() {
            for rf in &self.relations_to_fetch {
                if let Some(desc) = Selected::get_relation_descriptor(rf.relation) {
                    let needed_alias = if desc.is_has_many {
                        desc.current_primary_key_field_name
                    } else {
                        desc.foreign_key_field_name
                    };

                    // Check if this field is already requested
                    if !self.requested_aliases.iter().any(|a| a == needed_alias) {
                        if let Some(expr) = Selected::column_for_alias(needed_alias) {
                            selected.push((expr, needed_alias.to_string()));
                            defensive_fields.push(needed_alias.to_string());
                        }
                    }

                    // For now, we'll rely on the basic defensive field logic
                    // The macro-generated code will handle the defensive field fetching

                    // For has_many relations, also ensure we have the foreign key field from the target
                    if desc.is_has_many {
                        // Add any additional fields that might be needed for relation filtering
                        if let Some(nested_aliases) = &rf.nested_select_aliases {
                            for nested_alias in nested_aliases {
                                if !self.requested_aliases.iter().any(|a| a == nested_alias) {
                                    if let Some(expr) = Selected::column_for_alias(nested_alias) {
                                        selected.push((expr, nested_alias.clone()));
                                        defensive_fields.push(nested_alias.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Log defensive fields for debugging (can be removed in production)
        if !defensive_fields.is_empty() {
            // Debug logging can be enabled here if needed
        }

        let mut select = query.select_only();
        for (expr, alias) in &selected {
            select.expr_as(expr.clone(), alias.as_str());
        }

        let stmt = select.build(self.database_backend);
        let entity_name = core::any::type_name::<Entity>();
        crate::hooks::emit_before(&crate::hooks::QueryEvent {
            builder: "SelectManyQueryBuilder",
            entity: entity_name,
            details: crate::hooks::compose_details("select_many", entity_name),
        });
        let start = std::time::Instant::now();
        let rows_res = self.conn.query_all(stmt).await;
        match rows_res {
            Ok(rows) => {
                crate::hooks::emit_after(
                    &crate::hooks::QueryEvent {
                        builder: "SelectManyQueryBuilder",
                        entity: entity_name,
                        details: crate::hooks::compose_details("select_many", entity_name),
                    },
                    &crate::hooks::QueryResultMeta {
                        row_count: Some(rows.len()),
                        error: None,
                        elapsed_ms: Some(start.elapsed().as_millis()),
                    },
                );
                let mut out: Vec<Selected> = Vec::with_capacity(rows.len());
                let field_names: Vec<&str> =
                    self.requested_aliases.iter().map(|a| a.as_str()).collect();

                for row in rows.into_iter() {
                    let mut s = Selected::fill_from_row(&row, &field_names);

                    if !self.relations_to_fetch.is_empty() {
                        for rel in Selected::relation_descriptors() {
                            let needed_key = if rel.is_has_many {
                                rel.current_primary_key_field_name
                            } else {
                                rel.foreign_key_field_name
                            };
                            if s.get_key(needed_key).is_some() {
                            } else {
                                continue;
                            }
                        }
                        for rf in &self.relations_to_fetch {
                            <Selected as ApplyNestedIncludes<C>>::apply_relation_filter(
                                &mut s,
                                self.conn,
                                rf,
                                self.registry,
                            )
                            .await?;
                        }
                    }

                    // clear_unselected no longer needed - fields are only populated if selected
                    out.push(s);
                }
                Ok(out)
            }
            Err(e) => {
                crate::hooks::emit_after(
                    &crate::hooks::QueryEvent {
                        builder: "SelectManyQueryBuilder",
                        entity: entity_name,
                        details: crate::hooks::compose_details("select_many", entity_name),
                    },
                    &crate::hooks::QueryResultMeta {
                        row_count: None,
                        error: Some(e.to_string()),
                        elapsed_ms: Some(start.elapsed().as_millis()),
                    },
                );
                Err(e)
            }
        }
    }

    /// Execute and return custom selection types
    pub async fn exec_custom<CustomType>(self) -> Result<Vec<CustomType>, sea_orm::DbErr>
    where
        CustomType: From<Selected>,
    {
        let results = self.exec().await?;
        Ok(results.into_iter().map(CustomType::from).collect())
    }

    /// Add a relation to fetch with the selection
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }
}

impl<'a, C, Entity, Selected> SelectManyQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected:
        EntitySelection + HasRelationMetadata<Selected> + ApplyNestedIncludes<C> + Send + 'static,
{
    /// Accept a typed selection spec generated by per-entity select! macro
    pub fn select<S>(mut self, spec: S) -> SelectManyQueryBuilder<'a, C, Entity, S::Data>
    where
        S: SelectionSpec<Entity = Entity>,
        S::Data: EntitySelection
            + HasRelationMetadata<S::Data>
            + ApplyNestedIncludes<C>
            + Send
            + 'static,
    {
        let aliases = spec.collect_aliases();
        let mut requested = Vec::new();
        for alias in &aliases {
            if let Some(expr) = Selected::column_for_alias(alias.as_str()) {
                self.selected_fields.push((expr, alias.clone()));
                requested.push(alias.clone());
            }
        }
        SelectManyQueryBuilder {
            query: self.query,
            conn: self.conn,
            selected_fields: self.selected_fields,
            requested_aliases: requested,
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
            _phantom: std::marker::PhantomData::<S::Data>,
        }
    }
}

impl<'a, C, Entity, Selected> From<crate::query_builders::ManyQueryBuilder<'a, C, Entity, Selected>>
    for SelectManyQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    fn from(src: crate::query_builders::ManyQueryBuilder<'a, C, Entity, Selected>) -> Self {
        SelectManyQueryBuilder {
            query: src.query,
            conn: src.conn,
            selected_fields: Vec::new(),
            requested_aliases: Vec::new(),
            relations_to_fetch: src.relations_to_fetch,
            registry: src.registry,
            database_backend: src.database_backend,
            reverse_order: src.reverse_order,
            pending_order_bys: src.pending_order_bys,
            pending_nulls: None,
            cursor: src.cursor,
            is_distinct: src.is_distinct,
            distinct_on_fields: src.distinct_on_fields,
            distinct_on_columns: src.distinct_on_columns,
            skip_is_negative: false,
            _phantom: std::marker::PhantomData,
        }
    }
}
