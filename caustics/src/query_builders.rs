use crate::{
    EntityRegistry, FromModel, HasRelationMetadata, MergeInto, RelationFetcher, RelationFilter,
};
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Select,
};

use std::any::Any;
// Remove: use caustics::QueryMode;
// Use crate::QueryMode instead in the function if needed.

/// Trait to make DeferredLookup work with both regular connections and transactions
pub trait ConnectionLike: ConnectionTrait {}

impl<T: ConnectionTrait> ConnectionLike for T {}

/// Internal structure for storing deferred foreign key lookups
pub struct DeferredLookup<C: ConnectionTrait> {
    pub unique_param: Box<dyn Any + Send>,
    pub assign: fn(&mut (dyn Any + 'static), i32),
    pub entity_resolver: Box<
        dyn for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send,
    >,
}

impl<C: ConnectionTrait> DeferredLookup<C> {
    pub fn new(
        unique_param: Box<dyn Any + Send>,
        assign: fn(&mut (dyn Any + 'static), i32),
        entity_resolver: impl for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send
            + 'static,
    ) -> Self {
        Self {
            unique_param,
            assign,
            entity_resolver: Box::new(entity_resolver),
        }
    }
}

/// Query builder for finding a unique entity record
pub struct UniqueQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    UniqueQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
        if self.relations_to_fetch.is_empty() {
            // No relations to fetch, use simple query
            self.query
                .one(self.conn)
                .await
                .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
        } else {
            // Fetch with relations
            self.exec_with_relations().await
        }
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let Self {
            query,
            conn,
            relations_to_fetch,
            registry,
            ..
        } = self;
        let main_result = query.one(conn).await?;

        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);

            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(
                    conn,
                    &mut model_with_relations,
                    relation_filter.relation,
                    &relation_filter.filters,
                    registry,
                )
                .await?;
            }

            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relation_name: &str,
        _filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Convert relation_name to snake_case for lookup
        let relation_name_snake = heck::ToSnakeCase::to_snake_case(relation_name);
        let descriptor = ModelWithRelations::get_relation_descriptor(&relation_name_snake)
            .ok_or_else(|| {
                sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name))
            })?;

        // Always use the current entity's name for the fetcher
        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();
        let fetcher = registry.get_fetcher(&fetcher_entity_name).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "No fetcher found for entity: {}",
                fetcher_entity_name
            ))
        })?;

        // Fetch the relation data
        let fetched_result = fetcher
            .fetch_by_foreign_key(
                conn,
                (descriptor.get_foreign_key)(model_with_relations),
                descriptor.foreign_key_column,
                &fetcher_entity_name,
                relation_name,
            )
            .await?;

        // The fetcher already returns the correct type, just pass it directly
        (descriptor.set_field)(model_with_relations, fetched_result);

        Ok(())
    }
}

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: sea_orm::DatabaseBackend,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
        if self.relations_to_fetch.is_empty() {
            // No relations to fetch, use simple query
            self.query
                .one(self.conn)
                .await
                .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
        } else {
            // Fetch with relations
            self.exec_with_relations().await
        }
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let Self {
            query,
            conn,
            relations_to_fetch,
            registry,
            ..
        } = self;
        let main_result = query.one(conn).await?;

        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);

            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(
                    conn,
                    &mut model_with_relations,
                    relation_filter.relation,
                    &relation_filter.filters,
                    registry,
                )
                .await?;
            }

            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relation_name: &str,
        _filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Use the actual relation fetcher implementation
        let descriptor =
            ModelWithRelations::get_relation_descriptor(relation_name).ok_or_else(|| {
                sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name))
            })?;

        // Get the foreign key value from the model
        let foreign_key_value = (descriptor.get_foreign_key)(model_with_relations);

        // Get the target entity name from the descriptor
        let extracted_entity_name = extract_entity_name_from_path(&descriptor.target_entity);
        // Clone for use in fetcher_entity_name
        let extracted_entity_name = extracted_entity_name.clone();

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
        let fetcher = registry.get_fetcher(&fetcher_entity_name).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "No fetcher found for entity: {}",
                fetcher_entity_name
            ))
        })?;

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

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: sea_orm::DatabaseBackend,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Limit the number of results
    pub fn take(mut self, limit: u64) -> Self {
        self.query = self.query.limit(limit);
        self
    }

    /// Skip a number of results (for pagination)
    pub fn skip(mut self, offset: u64) -> Self {
        self.query = self.query.offset(offset);
        self
    }

    /// Order the results by a column
    pub fn order_by<Col>(mut self, col_and_order: impl Into<(Col, sea_orm::Order)>) -> Self
    where
        Col: sea_orm::ColumnTrait + sea_orm::IntoSimpleExpr,
    {
        let (col, order) = col_and_order.into();
        self.query = self.query.order_by(col, order);
        self
    }

    /// Execute the query and return multiple results
    pub async fn exec(self) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        if self.relations_to_fetch.is_empty() {
            // No relations to fetch, use simple query
            self.query.all(self.conn).await.map(|models| {
                models
                    .into_iter()
                    .map(|model| ModelWithRelations::from_model(model))
                    .collect()
            })
        } else {
            // Fetch with relations
            self.exec_with_relations().await
        }
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations(self) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let Self {
            query,
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
        _filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Use the actual relation fetcher implementation
        let descriptor =
            ModelWithRelations::get_relation_descriptor(relation_name).ok_or_else(|| {
                sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name))
            })?;

        // Get the foreign key value from the model
        let foreign_key_value = (descriptor.get_foreign_key)(model_with_relations);

        // Get the target entity name from the descriptor
        let extracted_entity_name = extract_entity_name_from_path(&descriptor.target_entity);
        // Clone for use in fetcher_entity_name
        let extracted_entity_name = extracted_entity_name.clone();

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
        let fetcher = registry.get_fetcher(&fetcher_entity_name).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "No fetcher found for entity: {}",
                fetcher_entity_name
            ))
        })?;

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

/// Query builder for creating a new entity record
pub struct CreateQueryBuilder<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations,
> {
    pub model: ActiveModel,
    pub conn: &'a C,
    pub deferred_lookups: Vec<DeferredLookup<C>>,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        let mut model = self.model;

        // Execute all deferred lookups in batch
        for lookup in &self.deferred_lookups {
            let lookup_result = (lookup.entity_resolver)(self.conn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        model
            .insert(self.conn)
            .await
            .map(ModelWithRelations::from_model)
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        let mut model = self.model;

        // Execute all deferred lookups in batch using the transaction
        for lookup in &self.deferred_lookups {
            // Cast the transaction to the expected connection type
            let conn_ref = unsafe { std::mem::transmute::<&DatabaseTransaction, &C>(txn) };
            let lookup_result = (lookup.entity_resolver)(conn_ref, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        model.insert(txn).await.map(ModelWithRelations::from_model)
    }
}

/// Query builder for deleting entity records matching a condition
pub struct DeleteQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: sea_orm::Condition,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<Entity>,
}

impl<'a, C, Entity> DeleteQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    pub async fn exec(self) -> Result<(), sea_orm::DbErr> {
        Entity::delete_many()
            .filter::<sea_orm::Condition>(self.condition)
            .exec(self.conn)
            .await?;
        Ok(())
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(self, txn: &DatabaseTransaction) -> Result<(), sea_orm::DbErr> {
        Entity::delete_many()
            .filter::<sea_orm::Condition>(self.condition)
            .exec(txn)
            .await?;
        Ok(())
    }
}

/// Query builder for upserting (insert or update) entity records
pub struct UpsertQueryBuilder<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations,
    T: MergeInto<ActiveModel>,
> {
    pub condition: sea_orm::Condition,
    pub create: (ActiveModel, Vec<DeferredLookup<C>>),
    pub update: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let existing = Entity::find()
            .filter::<sea_orm::Condition>(self.condition.clone())
            .one(self.conn)
            .await?;

        match existing {
            Some(active_model) => {
                let mut active_model = active_model.into_active_model();
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model
                    .update(self.conn)
                    .await
                    .map(ModelWithRelations::from_model)
            }
            None => {
                let (mut active_model, deferred_lookups) = self.create;
                // Execute all deferred lookups in batch (if needed)
                for lookup in &deferred_lookups {
                    let lookup_result =
                        (lookup.entity_resolver)(self.conn, &*lookup.unique_param).await?;
                    (lookup.assign)(&mut active_model as &mut (dyn Any + 'static), lookup_result);
                }
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model
                    .insert(self.conn)
                    .await
                    .map(ModelWithRelations::from_model)
            }
        }
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let existing = Entity::find()
            .filter::<sea_orm::Condition>(self.condition.clone())
            .one(txn)
            .await?;

        match existing {
            Some(active_model) => {
                let mut active_model = active_model.into_active_model();
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model
                    .update(txn)
                    .await
                    .map(ModelWithRelations::from_model)
            }
            None => {
                let (mut active_model, deferred_lookups) = self.create;
                // Execute all deferred lookups in batch (if needed)
                for lookup in &deferred_lookups {
                    // Cast the transaction to the expected connection type
                    let conn_ref = unsafe { std::mem::transmute::<&DatabaseTransaction, &C>(txn) };
                    let lookup_result =
                        (lookup.entity_resolver)(conn_ref, &*lookup.unique_param).await?;
                    (lookup.assign)(&mut active_model as &mut (dyn Any + 'static), lookup_result);
                }
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model
                    .insert(txn)
                    .await
                    .map(ModelWithRelations::from_model)
            }
        }
    }
}

/// Query builder for updating entity records
pub struct UpdateQueryBuilder<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations,
    T: MergeInto<ActiveModel>,
> {
    pub condition: sea_orm::Condition,
    pub changes: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let entity = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition)
            .one(self.conn)
            .await?;
        if let Some(entity) = entity {
            let mut active_model = entity.into_active_model();
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            active_model
                .update(self.conn)
                .await
                .map(ModelWithRelations::from_model)
        } else {
            Err(sea_orm::DbErr::RecordNotFound(
                "No record found to update".to_string(),
            ))
        }
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let entity = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition)
            .one(txn)
            .await?;
        if let Some(entity) = entity {
            let mut active_model = entity.into_active_model();
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            active_model
                .update(txn)
                .await
                .map(ModelWithRelations::from_model)
        } else {
            Err(sea_orm::DbErr::RecordNotFound(
                "No record found to update".to_string(),
            ))
        }
    }
}

/// Query builder for updates that include has_many set operations
pub struct HasManySetUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait + sea_orm::TransactionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel> + std::fmt::Debug,
{
    pub condition: sea_orm::Condition,
    pub changes: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    HasManySetUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait + sea_orm::TransactionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model> + crate::types::HasRelationMetadata<ModelWithRelations> + 'static,
    T: MergeInto<ActiveModel> + std::fmt::Debug + crate::types::SetParamInfo,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    /// Execute the update with has_many set operations
    pub async fn exec(mut self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        // Separate has_many set operations from regular changes
        let mut has_many_changes = Vec::new();
        let mut regular_changes = Vec::new();

        for change in std::mem::take(&mut self.changes) {
            if self.is_has_many_set_operation(&change) {
                has_many_changes.push(change);
            } else {
                regular_changes.push(change);
            }
        }

        // Extract entity ID from condition
        let entity_id = self.extract_entity_id_from_condition()?;

        // Process has_many set operations first
        if !has_many_changes.is_empty() {
            self.process_has_many_set_operations(has_many_changes, entity_id).await?;
        }

        // Then execute regular update
        let update_builder = UpdateQueryBuilder {
            condition: self.condition,
            changes: regular_changes,
            conn: self.conn,
            _phantom: std::marker::PhantomData,
        };

        update_builder.exec().await
    }

    /// Check if a change is a has_many set operation
    fn is_has_many_set_operation(&self, change: &T) -> bool {
        // Use proper trait-based pattern matching instead of string parsing
        change.is_has_many_set_operation()
    }

    /// Extract entity ID from the condition
    fn extract_entity_id_from_condition(&self) -> Result<sea_orm::Value, sea_orm::DbErr> {
        // This is a simplified implementation
        // In a real implementation, we'd parse the condition to extract the ID
        // For now, we'll use a basic approach that works for simple ID conditions
        
        // Convert condition to string and try to extract ID
        let condition_str = format!("{:?}", self.condition);
        
        // Try to extract ID using different patterns
        let extracted_id = Self::try_extract_id_pattern(&condition_str, "Id = ", 5)
            .or_else(|| Self::try_extract_id_pattern(&condition_str, "Value(Int(Some(", 15))
            .or_else(|| Self::try_extract_id_pattern(&condition_str, "IdEquals(", 9))
            .or_else(|| Self::try_extract_id_pattern(&condition_str, "Equal, Value(Int(Some(", 22))
            .or_else(|| Self::try_extract_id_pattern(&condition_str, "Value(Int(Some(", 15))
            .or_else(|| Self::try_extract_id_pattern(&condition_str, " = ", 3));
        
        match extracted_id {
            Some(id) => Ok(sea_orm::Value::Int(Some(id))),
            None => Err(sea_orm::DbErr::Custom(format!(
                "Could not extract entity ID from condition: {}",
                condition_str
            ))),
        }
    }
    
    /// Helper method to try extracting ID from a specific pattern
    fn try_extract_id_pattern(condition_str: &str, pattern: &str, pattern_len: usize) -> Option<i32> {
        condition_str.find(pattern).and_then(|id_start| {
            let after_pattern = &condition_str[id_start + pattern_len..];
            // Look for closing parenthesis or space
            let id_end = after_pattern.find(')').or_else(|| after_pattern.find(' '))?;
            let id_str = &after_pattern[..id_end];
            id_str.parse::<i32>().ok()
        })
    }

    /// Process has_many set operations
    async fn process_has_many_set_operations(
        &self,
        changes: Vec<T>,
        entity_id: sea_orm::Value,
    ) -> Result<(), sea_orm::DbErr> {
        // Use proper trait-based pattern matching instead of string parsing
        for change in changes {
            // Extract target IDs using the trait method
            let target_ids = change.extract_target_ids();
            
            // Extract relation name using the trait method
            let relation_name = change.extract_relation_name()
                .ok_or_else(|| sea_orm::DbErr::Custom("Could not extract relation name from change".to_string()))?;
            
            // Get relation metadata from the entity using the HasRelationMetadata trait
            let relation_metadata = <ModelWithRelations as crate::types::HasRelationMetadata<ModelWithRelations>>::get_relation_descriptor(relation_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(format!(
                    "No metadata found for relation: {}",
                    relation_name
                )))?;
            
            // Create handler using the metadata - completely agnostic of data model
            let handler = DefaultHasManySetHandler::new(
                relation_metadata.foreign_key_column.to_string(),
                relation_metadata.target_table_name.to_string(),
                relation_metadata.current_primary_key_column.to_string(),
                relation_metadata.target_primary_key_column.to_string(),
                relation_metadata.is_foreign_key_nullable,
            );
            
            handler.process_set_operation(self.conn, entity_id.clone(), target_ids).await?;
        }
        
        Ok(())
    }


}

/// SeaORM-specific relation fetcher implementation
pub struct SeaOrmRelationFetcher<R: EntityRegistry<C>, C: ConnectionTrait> {
    pub entity_registry: R,
    pub _phantom: std::marker::PhantomData<C>,
}

impl<C: ConnectionTrait, ModelWithRelations, R: EntityRegistry<C>>
    RelationFetcher<C, ModelWithRelations> for SeaOrmRelationFetcher<R, C>
where
    ModelWithRelations: HasRelationMetadata<ModelWithRelations> + 'static,
    R: Send + Sync,
    C: Send + Sync,
{
    fn fetch_relation_for_model(
        &self,
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relation_name: &str,
        _filters: &[crate::types::Filter],
    ) -> Result<(), sea_orm::DbErr> {
        // Look up the descriptor for this relation
        let descriptor =
            ModelWithRelations::get_relation_descriptor(relation_name).ok_or_else(|| {
                sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name))
            })?;

        // Always use the current entity's name for the fetcher
        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();

        // Use the entity registry to fetch the related entity
        if let Some(fetcher) = self.entity_registry.get_fetcher(&fetcher_entity_name) {
            // Create a runtime to execute the async operation
            let rt = tokio::runtime::Handle::current();
            let result = rt.block_on(async {
                fetcher
                    .fetch_by_foreign_key(
                        conn,
                        (descriptor.get_foreign_key)(model_with_relations),
                        descriptor.foreign_key_column,
                        &fetcher_entity_name,
                        relation_name,
                    )
                    .await
            })?;
            // Set the result on the model
            (descriptor.set_field)(model_with_relations, result);
            Ok(())
        } else {
            Err(sea_orm::DbErr::Custom(format!(
                "No fetcher found for entity: {}",
                fetcher_entity_name
            )))
        }
    }
}

/// Batch query types that can be executed in a transaction
pub enum BatchQuery<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations,
    T: MergeInto<ActiveModel>,
> {
    Insert(CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>),
    Update(UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
    Delete(DeleteQueryBuilder<'a, C, Entity>),
    Upsert(UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
}

/// Result types for batch operations
pub enum BatchResult<ModelWithRelations> {
    Insert(ModelWithRelations),
    Update(ModelWithRelations),
    Delete(()),
    Upsert(ModelWithRelations),
}

/// Extract entity name from a path string representation
fn extract_entity_name_from_path(path_str: &str) -> String {
    // The path is stored as a debug representation like:
    // "Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: \"super\", span: #11 bytes(117..128) }, arguments: PathArguments::None }, PathSep, PathSegment { ident: Ident { ident: \"user\", span: #11 bytes(117..128) }, arguments: PathArguments::None }] }"

    // Find all occurrences of "ident: \"entity_name\""
    let mut last_entity_name = "unknown".to_string();
    let mut pos = 0;

    while let Some(start) = path_str[pos..].find("ident: \"") {
        let full_start = pos + start + 8; // Skip "ident: \""
        if let Some(end) = path_str[full_start..].find("\"") {
            let entity_name = path_str[full_start..full_start + end].to_string();
            last_entity_name = entity_name;
            pos = full_start + end + 1;
        } else {
            break;
        }
    }

    last_entity_name
}

/// Generic trait for handling has_many set operations
pub trait HasManySetHandler<C>
where
    C: ConnectionTrait + sea_orm::TransactionTrait,
{
    /// Get the foreign key column name in the target entity
    fn foreign_key_column(&self) -> &str;
    
    /// Get the target table name
    fn target_table_name(&self) -> &str;
    
    /// Get the current entity's primary key column name
    fn current_primary_key_column(&self) -> &str;
    
    /// Get the target entity's primary key column name
    fn target_primary_key_column(&self) -> &str;
    
    /// Check if the foreign key is nullable
    fn is_foreign_key_nullable(&self) -> bool;
    
    /// Process the has_many set operation
    fn process_set_operation(
        &self,
        conn: &C,
        current_entity_id: sea_orm::Value,
        target_ids: Vec<sea_orm::Value>,
    ) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send;
}

/// Default implementation for has_many set operations
pub struct DefaultHasManySetHandler {
    foreign_key_column: String,
    target_table_name: String,
    current_primary_key_column: String,
    target_primary_key_column: String,
    is_foreign_key_nullable: bool,
}

impl DefaultHasManySetHandler {
    pub fn new(
        foreign_key_column: String,
        target_table_name: String,
        current_primary_key_column: String,
        target_primary_key_column: String,
        is_foreign_key_nullable: bool,
    ) -> Self {
        Self {
            foreign_key_column,
            target_table_name,
            current_primary_key_column,
            target_primary_key_column,
            is_foreign_key_nullable,
        }
    }
}

impl<C> HasManySetHandler<C> for DefaultHasManySetHandler
where
    C: ConnectionTrait + sea_orm::TransactionTrait,
{
    fn foreign_key_column(&self) -> &str {
        &self.foreign_key_column
    }
    
    fn target_table_name(&self) -> &str {
        &self.target_table_name
    }
    
    fn current_primary_key_column(&self) -> &str {
        &self.current_primary_key_column
    }
    
    fn target_primary_key_column(&self) -> &str {
        &self.target_primary_key_column
    }
    
    fn is_foreign_key_nullable(&self) -> bool {
        self.is_foreign_key_nullable
    }
    
    fn process_set_operation(
        &self,
        conn: &C,
        current_entity_id: sea_orm::Value,
        target_ids: Vec<sea_orm::Value>,
    ) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send {
        async move {
        let txn = conn.begin().await?;
        
        // Get the database backend from the connection
        let db_backend = conn.get_database_backend();
        
        // First, remove existing associations
        if self.is_foreign_key_nullable {
            // If nullable, set to NULL
            let remove_stmt = sea_orm::Statement::from_sql_and_values(
                db_backend,
                format!(
                    "UPDATE {} SET {} = NULL WHERE {} = ?",
                    self.target_table_name,
                    self.foreign_key_column,
                    self.foreign_key_column
                ),
                vec![current_entity_id.clone()]
            );
            txn.execute(remove_stmt).await?;
        } else {
            // For non-nullable foreign keys, we need to handle this more carefully
            // We'll use a smarter approach: only delete associations that are NOT in the target list
            
            if !target_ids.is_empty() {
                // Create placeholders for the target IDs
                let placeholders = target_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                
                // Delete existing associations that are NOT in the target list
                let delete_stmt = sea_orm::Statement::from_sql_and_values(
                    db_backend,
                    format!(
                        "DELETE FROM {} WHERE {} = ? AND {} NOT IN ({})",
                        self.target_table_name,
                        self.foreign_key_column,
                        self.target_primary_key_column,
                        placeholders
                    ),
                    {
                        let mut values = vec![current_entity_id.clone()];
                        values.extend(target_ids.clone());
                        values
                    }
                );

                txn.execute(delete_stmt).await?;
            } else {
                // If no target IDs, delete all existing associations
                let delete_stmt = sea_orm::Statement::from_sql_and_values(
                    db_backend,
                    format!(
                        "DELETE FROM {} WHERE {} = ?",
                        self.target_table_name,
                        self.foreign_key_column
                    ),
                    vec![current_entity_id.clone()]
                );

                txn.execute(delete_stmt).await?;
            }
        }
        
        // Then, set the target associations
        if !target_ids.is_empty() {
            let placeholders = target_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let set_query = format!(
                "UPDATE {} SET {} = ? WHERE {} IN ({})",
                self.target_table_name,
                self.foreign_key_column,
                self.target_primary_key_column,
                placeholders
            );
            
            let mut values = vec![current_entity_id];
            values.extend(target_ids.clone());
            
            
            
            let set_stmt = sea_orm::Statement::from_sql_and_values(
                db_backend,
                set_query,
                values
            );
            txn.execute(set_stmt).await?;
        }
        
        txn.commit().await?;
        Ok(())
        }
    }
}
