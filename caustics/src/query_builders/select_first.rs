use crate::types::{EntityRegistry, SelectionSpec};
use crate::{EntitySelection, HasRelationMetadata, RelationFilter};
use sea_orm::sea_query::SimpleExpr;
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QuerySelect, QueryTrait, Select};

/// Query builder for selected scalar fields on first
pub struct SelectFirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, Selected>
where
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub selected_fields: Vec<(SimpleExpr, String)>,
    pub requested_aliases: Vec<String>,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: DatabaseBackend,
    pub _phantom: std::marker::PhantomData<Selected>,
}

impl<'a, C, Entity, Selected> SelectFirstQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub fn push_field(mut self, expr: SimpleExpr, alias: &str) -> Self {
        self.selected_fields.push((expr, alias.to_string()));
        self
    }

    /// Execute and return selected row with type inference
    pub async fn exec<T>(self) -> Result<Option<T>, sea_orm::DbErr>
    where
        T: From<Selected>,
    {
        match self.exec_internal().await? {
            Some(selected) => Ok(Some(T::from(selected))),
            None => Ok(None),
        }
    }

    /// Internal implementation for exec
    async fn exec_internal(self) -> Result<Option<Selected>, sea_orm::DbErr> {
        // Ensure required key columns for any requested relations are added implicitly via Selected::column_for_alias
        let query = self.query.clone();
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
            // println!("Defensive fields added for relations: {:?}", defensive_fields);
        }
        let mut select = query.select_only();
        for (expr, alias) in &selected {
            select.expr_as(expr.clone(), alias.as_str());
        }
        let stmt = select.build(self.database_backend);
        let entity_name = core::any::type_name::<Entity>();
        crate::hooks::emit_before(&crate::hooks::QueryEvent {
            builder: "SelectFirstQueryBuilder",
            entity: entity_name,
            details: crate::hooks::compose_details("select_first", entity_name),
        });
        let start = std::time::Instant::now();
        if let Some(row) = self.conn.query_one(stmt).await? {
            let field_names: Vec<&str> =
                self.requested_aliases.iter().map(|a| a.as_str()).collect();
            let mut s = Selected::fill_from_row(&row, &field_names);

            for rf in &self.relations_to_fetch {
                if let Some(desc) = Selected::get_relation_descriptor(rf.relation) {
                    let fk_val = if desc.is_has_many {
                        s.get_key(desc.current_primary_key_field_name)
                    } else {
                        s.get_key(desc.foreign_key_field_name)
                    };
                    if let Some(fk) = fk_val {
                        let fetcher =
                            self.registry
                                .get_fetcher(desc.target_entity)
                                .ok_or_else(|| {
                                    crate::types::CausticsError::EntityFetcherMissing {
                                        entity: desc.target_entity.to_string(),
                                    }
                                })?;
                        let res = fetcher
                            .fetch_by_foreign_key_with_selection(
                                self.conn,
                                Some(fk),
                                desc.foreign_key_column,
                                desc.target_entity,
                                rf.relation,
                                rf,
                            )
                            .await?;
                        s.set_relation(rf.relation, res);
                    }
                }
            }

            // clear_unselected no longer needed - fields are only populated if selected
            crate::hooks::emit_after(
                &crate::hooks::QueryEvent {
                    builder: "SelectFirstQueryBuilder",
                    entity: entity_name,
                    details: crate::hooks::compose_details("select_first", entity_name),
                },
                &crate::hooks::QueryResultMeta {
                    row_count: Some(1),
                    error: None,
                    elapsed_ms: Some(start.elapsed().as_millis()),
                },
            );
            Ok(Some(s))
        } else {
            crate::hooks::emit_after(
                &crate::hooks::QueryEvent {
                    builder: "SelectFirstQueryBuilder",
                    entity: entity_name,
                    details: crate::hooks::compose_details("select_first", entity_name),
                },
                &crate::hooks::QueryResultMeta {
                    row_count: Some(0),
                    error: None,
                    elapsed_ms: Some(start.elapsed().as_millis()),
                },
            );
            Ok(None)
        }
    }

    /// Execute and return custom selection types
    pub async fn exec_custom<CustomType>(self) -> Result<Option<CustomType>, sea_orm::DbErr>
    where
        CustomType: From<Selected>,
    {
        match self.exec().await? {
            Some(selected) => Ok(Some(CustomType::from(selected))),
            None => Ok(None),
        }
    }

    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }
}

impl<'a, C, Entity, Selected>
    From<crate::query_builders::FirstQueryBuilder<'a, C, Entity, Selected>>
    for SelectFirstQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    fn from(src: crate::query_builders::FirstQueryBuilder<'a, C, Entity, Selected>) -> Self {
        SelectFirstQueryBuilder {
            query: src.query,
            conn: src.conn,
            selected_fields: Vec::new(),
            requested_aliases: Vec::new(),
            relations_to_fetch: src.relations_to_fetch,
            registry: src.registry,
            database_backend: src.database_backend,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, C, Entity, Selected> SelectFirstQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub fn select<S>(mut self, spec: S) -> SelectFirstQueryBuilder<'a, C, Entity, S::Data>
    where
        S: SelectionSpec<Entity = Entity>,
        S::Data: EntitySelection + HasRelationMetadata<S::Data> + Send + 'static,
    {
        let aliases = spec.collect_aliases();
        let mut requested = Vec::new();
        for alias in &aliases {
            if let Some(expr) = Selected::column_for_alias(alias.as_str()) {
                self.selected_fields.push((expr, alias.clone()));
                requested.push(alias.clone());
            }
        }
        SelectFirstQueryBuilder {
            query: self.query,
            conn: self.conn,
            selected_fields: self.selected_fields,
            requested_aliases: requested,
            relations_to_fetch: self.relations_to_fetch,
            registry: self.registry,
            database_backend: self.database_backend,
            _phantom: std::marker::PhantomData::<S::Data>,
        }
    }
}
