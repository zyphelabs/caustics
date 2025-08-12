use crate::{FromModel, HasRelationMetadata, RelationFilter, types};
use crate::types::{EntityRegistry, CausticsError};
use heck::ToSnakeCase;
use sea_orm::{ConnectionTrait, EntityTrait, Select};

/// Query builder for finding a unique entity record
pub struct UniqueQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    UniqueQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model> + HasRelationMetadata<ModelWithRelations> + Send + 'static,
{
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
        if self.relations_to_fetch.is_empty() {
            self.query
                .one(self.conn)
                .await
                .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
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
    async fn exec_with_relations(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
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
        let main_result = query.one(conn).await?;

        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);

            // Fetch relations for the main model
            for relation_filter in relations_to_fetch {
                Self::fetch_relation_for_model(
                    conn,
                    &mut model_with_relations,
                    relation_filter.relation,
                    &relation_filter.filters,
                    registry,
                )
                .await?;
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
        _filters: &[types::Filter],
        registry: &dyn EntityRegistry<C>,
    ) -> Result<(), sea_orm::DbErr> {
        // Convert relation_name to snake_case for lookup
        let relation_name_snake = ToSnakeCase::to_snake_case(relation_name);
        let descriptor = ModelWithRelations::get_relation_descriptor(&relation_name_snake)
            .ok_or_else(|| CausticsError::RelationNotFound { relation: relation_name.to_string() })?;

        // Always use the current entity's name for the fetcher
        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();
        let fetcher = registry.get_fetcher(&fetcher_entity_name)
            .ok_or_else(|| CausticsError::EntityFetcherMissing { entity: fetcher_entity_name.clone() })?;

        // Fetch the relation data
        let fetched_result = fetcher
            .fetch_by_foreign_key(
                conn,
                (descriptor.get_foreign_key)(model_with_relations),
                descriptor.foreign_key_column,
                &fetcher_entity_name,
                relation_name,
            )
            .await?;

        // The fetcher already returns the correct type, just pass it directly
        (descriptor.set_field)(model_with_relations, fetched_result);

        Ok(())
    }
}

