use sea_orm::{ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter};

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

