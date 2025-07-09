use sea_orm::{ConnectionTrait, EntityTrait, Select, Condition, QuerySelect, QueryOrder, QueryTrait, ColumnTrait, IntoSimpleExpr, IntoActiveModel, ActiveModelTrait, ActiveModelBehavior, QueryFilter};
use crate::{EntityRegistry, EntityFetcher, RelationFetcher, RelationFilterTrait, Filter, FromModel, HasRelationMetadata, MergeInto, RelationFilter};

use std::any::Any;

/// Query builder for finding a unique entity record
pub struct UniqueQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> 
where
    ModelWithRelations: FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
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
        let Self { query, conn, relations_to_fetch, registry, .. } = self;
        let main_result = query.one(conn).await?;
        
        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            
            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
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
        filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Convert relation_name to snake_case for lookup
        let relation_name_snake = heck::ToSnakeCase::to_snake_case(relation_name);
        let descriptor = ModelWithRelations::get_relation_descriptor(&relation_name_snake)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name)))?;
        
        // Always use the current entity's name for the fetcher
        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();
        println!("DEBUG: fetch_relation_for_model - ModelWithRelations type: {} | relation_name: {} | fetcher_entity_name: {}", type_name, relation_name, fetcher_entity_name);
        println!("DEBUG: About to call get_fetcher with entity name: '{}'", fetcher_entity_name);
        let fetcher = registry.get_fetcher(&fetcher_entity_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", fetcher_entity_name)))?;
        // Fetch the relation data
        let fetched_result = fetcher.fetch_by_foreign_key(
            conn, 
            (descriptor.get_foreign_key)(model_with_relations), 
            descriptor.foreign_key_column, 
            &fetcher_entity_name,
            relation_name
        ).await?;
        (descriptor.set_field)(model_with_relations, fetched_result);
        Ok(())
    }

    /// Fetch relations for the model
    async fn fetch_relations(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relations_to_fetch: &[RelationFilter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Fetch relations for the main model
        for relation_filter in relations_to_fetch {
            Self::fetch_relation_for_model(conn, model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
        }
        
        Ok(())
    }
}

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> FirstQueryBuilder<'a, C, Entity, ModelWithRelations> 
where
    ModelWithRelations: FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
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
        let Self { query, conn, relations_to_fetch, registry, .. } = self;
        let main_result = query.one(conn).await?;
        
        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            
            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
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
        filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Use the actual relation fetcher implementation
        let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name)))?;
        
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
        println!("DEBUG: About to call get_fetcher with entity name: '{}'", fetcher_entity_name);
        let fetcher = registry.get_fetcher(&fetcher_entity_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", fetcher_entity_name)))?;
        
        // Fetch the relation data
        let fetched_result = fetcher.fetch_by_foreign_key(
            conn, 
            foreign_key_value, 
            foreign_key_column, 
            &fetcher_entity_name,
            relation_name
        ).await?;
        
        // The fetcher already returns the correct type, just pass it directly
        println!("DEBUG: fetched_result is_some: {}", fetched_result.is::<Option<Vec<()>>>());
        println!("DEBUG: fetched_result is_unit: {}", fetched_result.is::<()>());
        println!("DEBUG: relation_name: {}", relation_name);
        println!("DEBUG: target_entity_name: {}", extracted_entity_name);
        (descriptor.set_field)(model_with_relations, fetched_result);
        
        Ok(())
    }

    /// Fetch relations for the model
    async fn fetch_relations(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relations_to_fetch: &[RelationFilter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Fetch relations for the main model
        for relation_filter in relations_to_fetch {
            Self::fetch_relation_for_model(conn, model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
        }
        
        Ok(())
    }
}

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> ManyQueryBuilder<'a, C, Entity, ModelWithRelations> 
where
    ModelWithRelations: FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
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
        let Self { query, conn, relations_to_fetch, registry, .. } = self;
        let main_results = query.all(conn).await?;
        
        let mut models_with_relations = Vec::new();
        
        for main_model in main_results {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);
            for relation_filter in &relations_to_fetch {
                Self::fetch_relation_for_model(conn, &mut model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
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
        filters: &[crate::types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Use the actual relation fetcher implementation
        let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name)))?;
        
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
        println!("DEBUG: About to call get_fetcher with entity name: '{}'", fetcher_entity_name);
        let fetcher = registry.get_fetcher(&fetcher_entity_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", fetcher_entity_name)))?;
        
        // Fetch the relation data
        let fetched_result = fetcher.fetch_by_foreign_key(
            conn, 
            foreign_key_value, 
            foreign_key_column, 
            &fetcher_entity_name,
            relation_name
        ).await?;
        
        // The fetcher already returns the correct type, just pass it directly
        println!("DEBUG: fetched_result is_some: {}", fetched_result.is::<Option<Vec<()>>>());
        println!("DEBUG: fetched_result is_unit: {}", fetched_result.is::<()>());
        println!("DEBUG: relation_name: {}", relation_name);
        println!("DEBUG: target_entity_name: {}", extracted_entity_name);
        (descriptor.set_field)(model_with_relations, fetched_result);
        
        Ok(())
    }

    /// Fetch relations for the model
    async fn fetch_relations(
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relations_to_fetch: &[RelationFilter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Fetch relations for the main model
        for relation_filter in relations_to_fetch {
            Self::fetch_relation_for_model(conn, model_with_relations, relation_filter.relation, &relation_filter.filters, registry).await?;
        }
        
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
            .filter::<sea_orm::Condition>(self.condition)
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
            .filter::<sea_orm::Condition>(self.condition.clone())
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
        let entity = <Entity as EntityTrait>::find().filter::<sea_orm::Condition>(self.condition).one(self.conn).await?;
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

/// SeaORM-specific relation fetcher implementation
pub struct SeaOrmRelationFetcher<R: EntityRegistry<C>, C: ConnectionTrait> {
    pub entity_registry: R,
    pub _phantom: std::marker::PhantomData<C>,
}

impl<C: ConnectionTrait, ModelWithRelations, R: EntityRegistry<C>> RelationFetcher<C, ModelWithRelations> 
for SeaOrmRelationFetcher<R, C> 
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
        filters: &[crate::types::Filter],
    ) -> Result<(), sea_orm::DbErr> {
        // Look up the descriptor for this relation
        let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
            .ok_or_else(|| sea_orm::DbErr::Custom(format!("Relation '{}' not found", relation_name)))?;

        // Always use the current entity's name for the fetcher
        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();
        println!("DEBUG: ModelWithRelations type_name: {} | fetcher_entity_name: {}", type_name, fetcher_entity_name);

        // Use the entity registry to fetch the related entity
        if let Some(fetcher) = self.entity_registry.get_fetcher(&fetcher_entity_name) {
            // Create a runtime to execute the async operation
            let rt = tokio::runtime::Handle::current();
            let result = rt.block_on(async {
                fetcher.fetch_by_foreign_key(conn, (descriptor.get_foreign_key)(model_with_relations), descriptor.foreign_key_column, &fetcher_entity_name, relation_name).await
            })?;
            // Set the result on the model
            (descriptor.set_field)(model_with_relations, result);
            Ok(())
        } else {
            Err(sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", fetcher_entity_name)))
        }
    }
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