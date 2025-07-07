use sea_orm::{ConnectionTrait, EntityTrait, Select, QuerySelect, QueryOrder, QueryFilter, IntoActiveModel};

use crate::{FromModel, MergeInto, RelationFilterTrait, RelationFilter};

use std::any::Any;

/// Query builder for finding a unique entity record
pub struct UniqueQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
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
        let Self { query, conn, relations_to_fetch, .. } = self;
        let main_result = query.one(conn).await?;
        
        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            
            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, &relation_filter).await?;
            }
            
            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        _conn: &C,
        _model_with_relations: &mut ModelWithRelations,
        relation_filter: &RelationFilter,
    ) -> Result<(), sea_orm::DbErr> {
        // This is a placeholder implementation
        // In a real implementation, you would:
        // 1. Use the relation name to determine which relation to fetch
        // 2. Use SeaORM's relation API to fetch related data
        // 3. Apply any filters from the relation_filter
        // 4. Populate the appropriate field in model_with_relations
        
        let relation_name = relation_filter.relation_name();
        let filters = relation_filter.filters();
        
        // For now, we'll just log what we would fetch
        println!("Would fetch relation '{}' with {} filters", relation_name, filters.len());
        
        // TODO: Implement actual relation fetching logic
        // This would involve:
        // - Using SeaORM's RelationTrait to get the related entity
        // - Building a query with the filters
        // - Executing the query and populating the relation field
        
        Ok(())
    }
}

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
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
        let Self { query, conn, relations_to_fetch, .. } = self;
        let main_result = query.one(conn).await?;
        
        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            
            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, &relation_filter).await?;
            }
            
            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        _conn: &C,
        _model_with_relations: &mut ModelWithRelations,
        relation_filter: &RelationFilter,
    ) -> Result<(), sea_orm::DbErr> {
        // This is a placeholder implementation
        // In a real implementation, you would:
        // 1. Use the relation name to determine which relation to fetch
        // 2. Use SeaORM's relation API to fetch related data
        // 3. Apply any filters from the relation_filter
        // 4. Populate the appropriate field in model_with_relations
        
        let relation_name = relation_filter.relation_name();
        let filters = relation_filter.filters();
        
        // For now, we'll just log what we would fetch
        println!("Would fetch relation '{}' with {} filters", relation_name, filters.len());
        
        // TODO: Implement actual relation fetching logic
        // This would involve:
        // - Using SeaORM's RelationTrait to get the related entity
        // - Building a query with the filters
        // - Executing the query and populating the relation field
        
        Ok(())
    }
}

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
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
            self.query
                .all(self.conn)
                .await
                .map(|models| models.into_iter().map(|model| ModelWithRelations::from_model(model)).collect())
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
        let Self { query, conn, relations_to_fetch, .. } = self;
        let main_results = query.all(conn).await?;
        
        let mut models_with_relations = Vec::new();
        
        for main_model in main_results {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            for relation_filter in &relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, relation_filter).await?;
            }
            models_with_relations.push(model_with_relations);
        }
        
        Ok(models_with_relations)
    }

    /// Fetch a single relation for a model
    async fn fetch_relation_for_model(
        _conn: &C,
        _model_with_relations: &mut ModelWithRelations,
        relation_filter: &RelationFilter,
    ) -> Result<(), sea_orm::DbErr> {
        // This is a placeholder implementation
        // In a real implementation, you would:
        // 1. Use the relation name to determine which relation to fetch
        // 2. Use SeaORM's relation API to fetch related data
        // 3. Apply any filters from the relation_filter
        // 4. Populate the appropriate field in model_with_relations
        
        let relation_name = relation_filter.relation_name();
        let filters = relation_filter.filters();
        
        // For now, we'll just log what we would fetch
        println!("Would fetch relation '{}' with {} filters", relation_name, filters.len());
        
        // TODO: Implement actual relation fetching logic
        // This would involve:
        // - Using SeaORM's RelationTrait to get the related entity
        // - Building a query with the filters
        // - Executing the query and populating the relation field
        
        Ok(())
    }
}

/// Internal structure for storing deferred foreign key lookups
pub struct DeferredLookup<C: ConnectionTrait> {
    pub unique_param: Box<dyn Any + Send>,
    pub assign: fn(&mut (dyn Any + 'static), i32),
    pub entity_resolver: Box<
        dyn for<'a> Fn(&'a C, &dyn Any) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<i32, sea_orm::DbErr>> + Send + 'a>>
        + Send
    >,
}

/// Query builder for creating a new entity record
pub struct CreateQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send + 'static, ModelWithRelations> {
    pub model: ActiveModel,
    pub conn: &'a C,
    pub deferred_lookups: Vec<DeferredLookup<C>>,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations> CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
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
        
        model.insert(self.conn).await.map(ModelWithRelations::from_model)
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
            .filter(self.condition)
            .exec(self.conn)
            .await?;
        Ok(())
    }
}

/// Query builder for upserting (insert or update) entity records
pub struct UpsertQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send + 'static, ModelWithRelations, T: MergeInto<ActiveModel>> {
    pub condition: sea_orm::Condition,
    pub create: (ActiveModel, Vec<DeferredLookup<C>>),
    pub update: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T> UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let existing = Entity::find()
            .filter(self.condition.clone())
            .one(self.conn)
            .await?;

        match existing {
            Some(active_model) => {
                let mut active_model = active_model.into_active_model();
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model.update(self.conn).await.map(ModelWithRelations::from_model)
            }
            None => {
                let (mut active_model, deferred_lookups) = self.create;
                // Execute all deferred lookups in batch (if needed)
                for lookup in &deferred_lookups {
                    let lookup_result = (lookup.entity_resolver)(self.conn, &*lookup.unique_param).await?;
                    (lookup.assign)(&mut active_model as &mut (dyn Any + 'static), lookup_result);
                }
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model.insert(self.conn).await.map(ModelWithRelations::from_model)
            }
        }
    }


}

/// Query builder for updating entity records
pub struct UpdateQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send, ModelWithRelations, T: MergeInto<ActiveModel>> {
    pub condition: sea_orm::Condition,
    pub changes: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T> UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let entity = <Entity as EntityTrait>::find().filter(self.condition).one(self.conn).await?;
        if let Some(entity) = entity {
            let mut active_model = entity.into_active_model();
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            active_model.update(self.conn).await.map(ModelWithRelations::from_model)
        } else {
            Err(sea_orm::DbErr::RecordNotFound("No record found to update".to_string()))
        }
    }
}