use super::has_many_set::HasManySetUpdateQueryBuilder;
use crate::{FromModel, MergeInto, RelationFilter, ApplyNestedIncludes, HasRelationMetadata, EntityRegistry};
use sea_orm::{ConnectionTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter};

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
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel, ModelWithRelations)>,
}

/// Unified update builder that can handle either scalar field updates or has_many set relation updates
pub enum UnifiedUpdateQueryBuilder<
    'a,
    C: ConnectionTrait + sea_orm::TransactionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel> + std::fmt::Debug + crate::types::SetParamInfo,
    P: crate::EntityMetadataProvider,
> {
    Scalar(UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
    Relations(HasManySetUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T, P>),
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T, P>
    UnifiedUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T, P>
where
    C: ConnectionTrait + sea_orm::TransactionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
        + crate::types::HasRelationMetadata<ModelWithRelations>
        + 'static,
    T: MergeInto<ActiveModel> + std::fmt::Debug + crate::types::SetParamInfo,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    P: crate::EntityMetadataProvider,
{
    pub fn with<R: Into<RelationFilter>>(self, relation: R) -> Self {
        match self {
            UnifiedUpdateQueryBuilder::Scalar(mut b) => {
                b.relations_to_fetch.push(relation.into());
                UnifiedUpdateQueryBuilder::Scalar(b)
            }
            UnifiedUpdateQueryBuilder::Relations(mut b) => {
                b.relations_to_fetch.push(relation.into());
                UnifiedUpdateQueryBuilder::Relations(b)
            }
        }
    }
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        ModelWithRelations: ApplyNestedIncludes<C>,
    {
        match self {
            UnifiedUpdateQueryBuilder::Scalar(b) => b.exec().await,
            UnifiedUpdateQueryBuilder::Relations(b) => b.exec().await,
        }
    }

    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr> {
        match self {
            UnifiedUpdateQueryBuilder::Scalar(b) => b.exec_in_txn(txn).await,
            UnifiedUpdateQueryBuilder::Relations(b) => b.exec_in_txn(txn).await,
        }
    }
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
        + HasRelationMetadata<ModelWithRelations>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        ModelWithRelations: ApplyNestedIncludes<C>,
    {
        let cond_dbg = format!("{:?}", self.condition);
        let entity = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition)
            .one(self.conn)
            .await?;
        if let Some(entity) = entity {
            let mut active_model = entity.into_active_model();
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            let updated = active_model.update(self.conn).await?;
            let mut model_with_relations = ModelWithRelations::from_model(updated);

            if !self.relations_to_fetch.is_empty() {
                for relation_filter in self.relations_to_fetch {
                    ApplyNestedIncludes::apply_relation_filter(
                        &mut model_with_relations,
                        self.conn,
                        &relation_filter,
                        self.registry,
                    )
                    .await?;
                }
            }

            Ok(model_with_relations)
        } else {
            Err(crate::types::CausticsError::NotFoundForCondition {
                entity: core::any::type_name::<Entity>().to_string(),
                condition: cond_dbg,
            }
            .into())
        }
    }

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
    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let cond_dbg = format!("{:?}", self.condition);
        let entity = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition)
            .one(txn)
            .await?;
        if let Some(entity) = entity {
            let mut active_model = entity.into_active_model();
            for change in self.changes {
                change.merge_into(&mut active_model);
            }
            let updated = active_model.update(txn).await?;
            Ok(ModelWithRelations::from_model(updated))
        } else {
            Err(crate::types::CausticsError::NotFoundForCondition {
                entity: core::any::type_name::<Entity>().to_string(),
                condition: cond_dbg,
            }
            .into())
        }
    }
}
