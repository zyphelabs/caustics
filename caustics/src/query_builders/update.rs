use crate::{FromModel, MergeInto};
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

