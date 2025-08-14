use crate::{FromModel, HasRelationMetadata, RelationFilter};
use crate::types::ApplyNestedIncludes;
use crate::types::EntityRegistry;
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, Select};

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub database_backend: DatabaseBackend,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations:
        FromModel<Entity::Model>
        + HasRelationMetadata<ModelWithRelations>
        + crate::types::ApplyNestedIncludes<C>
        + Send
        + 'static,
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

            // Fetch relations for the main model (nested-aware)
            for relation_filter in relations_to_fetch {
                ApplyNestedIncludes::apply_relation_filter(&mut model_with_relations, conn, &relation_filter, registry).await?;
            }

            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }

}

