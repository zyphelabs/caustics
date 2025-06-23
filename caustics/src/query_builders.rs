use sea_orm::{ConnectionTrait, EntityTrait, Select, QuerySelect, QueryOrder, QueryFilter, IntoActiveModel};

use crate::{FromModel, MergeInto};

/// Query builder for finding a unique entity record
pub struct UniqueQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        self.query
            .one(self.conn)
            .await
            .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
    }

    /// Add a relation to fetch with the query
    pub fn with<T>(self, _relation: T) -> Self {
        // Stub implementation for now
        todo!("Implement .with() to fetch related rows matching the filter")
    }
}

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        self.query
            .one(self.conn)
            .await
            .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
    }

    /// Add a relation to fetch with the query
    pub fn with<T>(self, _relation: T) -> Self {
        // Stub implementation for now
        todo!("Implement .with() to fetch related rows matching the filter")
    }
}

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
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
        self.query
            .all(self.conn)
            .await
            .map(|models| models.into_iter().map(|model| ModelWithRelations::from_model(model)).collect())
    }

    /// Add a relation to fetch with the query
    pub fn with<T>(self, _relation: T) -> Self {
        // Stub implementation for now
        todo!("Implement .with() to fetch related rows matching the filter")
    }
}

/// Query builder for creating a new entity record
pub struct CreateQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send, ModelWithRelations> {
    pub model: ActiveModel,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations> CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        self.model.insert(self.conn).await.map(ModelWithRelations::from_model)
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
pub struct UpsertQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send, ModelWithRelations, T: MergeInto<ActiveModel>> {
    pub condition: sea_orm::Condition,
    pub create: ActiveModel,
    pub update: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T> UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity=Entity> + sea_orm::ActiveModelBehavior + Send,
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
            Some(model) => {
                let mut active_model = model.into_active_model();
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                active_model.update(self.conn).await.map(ModelWithRelations::from_model)
            }
            None => {
                let mut active_model = self.create;
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
        if let Some(model) = entity.map(|m| m.into_active_model()) {
            let mut active_model = model;
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            active_model.update(self.conn).await.map(ModelWithRelations::from_model)
        } else {
            Err(sea_orm::DbErr::RecordNotFound("No record found to update".to_string()))
        }
    }
}