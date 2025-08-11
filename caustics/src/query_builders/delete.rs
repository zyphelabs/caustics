use crate::FromModel;
use sea_orm::{ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter};

/// Query builder for deleting a single entity record matching a unique condition
pub struct DeleteQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub condition: sea_orm::Condition,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ModelWithRelations> DeleteQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    /// Delete the uniquely-matching record and return it; error if not found
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        // Fetch the record first so we can return it after deletion
        let found = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition.clone())
            .one(self.conn)
            .await?;

        if let Some(model) = found {
            // Delete the record using the same unique condition
            Entity::delete_many()
                .filter::<sea_orm::Condition>(self.condition)
                .exec(self.conn)
                .await?;
            Ok(ModelWithRelations::from_model(model))
        } else {
            Err(sea_orm::DbErr::RecordNotFound(
                "No record found to delete".to_string(),
            ))
        }
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(self, txn: &DatabaseTransaction) -> Result<ModelWithRelations, sea_orm::DbErr> {
        let found = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition.clone())
            .one(txn)
            .await?;

        if let Some(model) = found {
            Entity::delete_many()
                .filter::<sea_orm::Condition>(self.condition)
                .exec(txn)
                .await?;
            Ok(ModelWithRelations::from_model(model))
        } else {
            Err(sea_orm::DbErr::RecordNotFound(
                "No record found to delete".to_string(),
            ))
        }
    }
}

