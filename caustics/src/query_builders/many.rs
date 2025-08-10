use crate::{FromModel, HasRelationMetadata, RelationFilter};
use crate::types::{EntityRegistry, Filter};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QueryOrder, QuerySelect, Select};

/// Query builder for finding multiple entity records matching conditions
pub struct ManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: DatabaseBackend,
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
            self.query.all(self.conn).await.map(|models| {
                models
                    .into_iter()
                    .map(|model| ModelWithRelations::from_model(model))
                    .collect()
            })
        } else {
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
        _filters: &[Filter],
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
        let extracted_entity_name = super::utils::extract_entity_name_from_path(&descriptor.target_entity);
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

