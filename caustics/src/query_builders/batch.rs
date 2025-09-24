use crate::MergeInto;
use sea_orm::{ConnectionTrait, EntityTrait};

use super::{
    create::CreateQueryBuilder, delete::DeleteQueryBuilder, update::UpdateQueryBuilder,
    upsert::UpsertQueryBuilder,
};

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
    Delete(DeleteQueryBuilder<'a, C, Entity, ModelWithRelations>),
    Upsert(UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
}

/// Result types for batch operations
pub enum BatchResult<ModelWithRelations> {
    Insert(ModelWithRelations),
    Update(ModelWithRelations),
    Delete(ModelWithRelations),
    Upsert(ModelWithRelations),
}
