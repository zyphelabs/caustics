use crate::{FromModel, HasRelationMetadata, MergeInto};
use crate::types::SetParamInfo;
use sea_orm::{ConnectionTrait, DatabaseBackend, TransactionTrait, DatabaseTransaction};

/// Query builder for updates that include has_many set operations
pub struct HasManySetUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait + TransactionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel> + std::fmt::Debug,
{
    pub condition: sea_orm::Condition,
    pub changes: Vec<T>,
    pub conn: &'a C,
        pub entity_id_resolver: Option<Box<
            dyn for<'b> Fn(
                    &'b C,
                ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<Output = Result<sea_orm::Value, sea_orm::DbErr>>
                            + Send
                            + 'b,
                    >,
                > + Send,
        >>,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    HasManySetUpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait + TransactionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>
        + HasRelationMetadata<ModelWithRelations>
        + 'static,
    T: MergeInto<ActiveModel> + std::fmt::Debug + SetParamInfo,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    /// Execute the update with has_many set operations
    pub async fn exec(mut self) -> Result<ModelWithRelations, sea_orm::DbErr> {
        // Separate has_many set operations from regular changes
        let mut has_many_set_changes = Vec::new();
        let mut has_many_create_changes = Vec::new();
        let mut regular_changes = Vec::new();

        for change in std::mem::take(&mut self.changes) {
            if self.is_has_many_set_operation(&change) {
                has_many_set_changes.push(change);
            } else if change.is_has_many_create_operation() {
                has_many_create_changes.push(change);
            } else {
                regular_changes.push(change);
            }
        }

        // Resolve entity ID using typed resolver
        let entity_id = match &self.entity_id_resolver {
            Some(resolver) => (resolver)(self.conn).await?,
            None => {
                return Err(crate::types::CausticsError::QueryValidation { message: "Missing entity id resolver for has_many set".to_string() }.into())
            }
        };
        let parent_id_i32 = match entity_id {
            sea_orm::Value::Int(Some(id)) => id,
            _ => return Err(crate::types::CausticsError::QueryValidation { message: "Unsupported id type for has_many create".to_string() }.into()),
        };

        // Run nested creates, set operations, and regular update in a single transaction
        let txn: DatabaseTransaction = self.conn.begin().await?;

        if !has_many_create_changes.is_empty() {
            for change in has_many_create_changes {
                change.exec_has_many_create_on_txn(&txn, parent_id_i32).await?;
            }
        }

        if !has_many_set_changes.is_empty() {
            self
                .process_has_many_set_operations_in_txn(
                    has_many_set_changes,
                    sea_orm::Value::Int(Some(parent_id_i32)),
                    &txn,
                )
                .await?;
        }

        // Then execute regular update within the same transaction
        let update_builder = super::update::UpdateQueryBuilder {
            condition: self.condition,
            changes: regular_changes,
            conn: self.conn,
            _phantom: std::marker::PhantomData,
        };

        let result = update_builder.exec_in_txn(&txn).await?;
        txn.commit().await?;
        Ok(result)
    }

    /// Check if a change is a has_many set operation
    fn is_has_many_set_operation(&self, change: &T) -> bool {
        change.is_has_many_set_operation()
    }

    // entity id resolution is provided by codegen closure

    /// Process has_many set operations inside an existing transaction
    async fn process_has_many_set_operations_in_txn(
        &self,
        changes: Vec<T>,
        entity_id: sea_orm::Value,
        txn: &DatabaseTransaction,
    ) -> Result<(), sea_orm::DbErr> {
        for change in changes {
            let target_ids = change.extract_target_ids();
            let relation_name = change.extract_relation_name().ok_or_else(|| {
                sea_orm::DbErr::from(crate::types::CausticsError::QueryValidation { message: "Could not extract relation name from change".to_string() })
            })?;

            let relation_metadata = <ModelWithRelations as crate::types::HasRelationMetadata<
                ModelWithRelations,
            >>::get_relation_descriptor(relation_name)
            .ok_or_else(|| {
                sea_orm::DbErr::from(crate::types::CausticsError::RelationNotFound { relation: relation_name.to_string() })
            })?;

            let handler = DefaultHasManySetHandler::new(
                relation_metadata.foreign_key_column.to_string(),
                relation_metadata.target_table_name.to_string(),
                relation_metadata.current_primary_key_column.to_string(),
                relation_metadata.target_primary_key_column.to_string(),
                relation_metadata.is_foreign_key_nullable,
            );

            <DefaultHasManySetHandler as HasManySetHandler<C>>::process_set_operation_in_txn(
                &handler,
                txn,
                entity_id.clone(),
                target_ids,
            )
            .await?;
        }

        Ok(())
    }
}

/// Generic trait for handling has_many set operations
pub trait HasManySetHandler<C>
where
    C: ConnectionTrait + TransactionTrait,
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

    /// Process the has_many set operation inside an existing transaction
    fn process_set_operation_in_txn(
        &self,
        txn: &DatabaseTransaction,
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
    C: ConnectionTrait + TransactionTrait,
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
            let db_backend: DatabaseBackend = conn.get_database_backend();

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
                    vec![current_entity_id.clone()],
                );
                txn.execute(remove_stmt).await?;
            } else {
                // For non-nullable foreign keys, delete associations not in target list
                if !target_ids.is_empty() {
                    let placeholders = target_ids
                        .iter()
                        .map(|_| "?")
                        .collect::<Vec<_>>()
                        .join(",");

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
                        },
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
                        vec![current_entity_id.clone()],
                    );

                    txn.execute(delete_stmt).await?;
                }
            }

            // Then, set the target associations
            if !target_ids.is_empty() {
                let placeholders = target_ids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
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
                    values,
                );
                txn.execute(set_stmt).await?;
            }

            txn.commit().await?;
            Ok(())
        }
    }

    fn process_set_operation_in_txn(
        &self,
        txn: &DatabaseTransaction,
        current_entity_id: sea_orm::Value,
        target_ids: Vec<sea_orm::Value>,
    ) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send {
        let foreign_key_column = self.foreign_key_column.clone();
        let target_table_name = self.target_table_name.clone();
        let target_primary_key_column = self.target_primary_key_column.clone();
        let is_fk_nullable = self.is_foreign_key_nullable;
        async move {
            let db_backend: DatabaseBackend = sea_orm::ConnectionTrait::get_database_backend(txn);

            // First, remove existing associations
            if is_fk_nullable {
                // If nullable, set to NULL
                let remove_stmt = sea_orm::Statement::from_sql_and_values(
                    db_backend,
                    format!(
                        "UPDATE {} SET {} = NULL WHERE {} = ?",
                        target_table_name,
                        foreign_key_column,
                        foreign_key_column
                    ),
                    vec![current_entity_id.clone()],
                );
                <DatabaseTransaction as sea_orm::ConnectionTrait>::execute(txn, remove_stmt).await?;
            } else {
                // For non-nullable foreign keys, delete associations not in target list
                if !target_ids.is_empty() {
                    let placeholders = target_ids
                        .iter()
                        .map(|_| "?")
                        .collect::<Vec<_>>()
                        .join(",");

                    let delete_stmt = sea_orm::Statement::from_sql_and_values(
                        db_backend,
                        format!(
                            "DELETE FROM {} WHERE {} = ? AND {} NOT IN ({})",
                            target_table_name,
                            foreign_key_column,
                            target_primary_key_column,
                            placeholders
                        ),
                        {
                            let mut values = vec![current_entity_id.clone()];
                            values.extend(target_ids.clone());
                            values
                        },
                    );

                    <DatabaseTransaction as sea_orm::ConnectionTrait>::execute(txn, delete_stmt).await?;
                } else {
                    // If no target IDs, delete all existing associations
                    let delete_stmt = sea_orm::Statement::from_sql_and_values(
                        db_backend,
                        format!(
                            "DELETE FROM {} WHERE {} = ?",
                            target_table_name,
                            foreign_key_column
                        ),
                        vec![current_entity_id.clone()],
                    );

                    <DatabaseTransaction as sea_orm::ConnectionTrait>::execute(txn, delete_stmt).await?;
                }
            }

            // Then, set the target associations
            if !target_ids.is_empty() {
                let placeholders = target_ids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let set_query = format!(
                    "UPDATE {} SET {} = ? WHERE {} IN ({})",
                    target_table_name,
                    foreign_key_column,
                    target_primary_key_column,
                    placeholders
                );

                let mut values = vec![current_entity_id];
                values.extend(target_ids.clone());

                let set_stmt = sea_orm::Statement::from_sql_and_values(
                    db_backend,
                    set_query,
                    values,
                );
                <DatabaseTransaction as sea_orm::ConnectionTrait>::execute(txn, set_stmt).await?;
            }

            Ok(())
        }
    }
}

