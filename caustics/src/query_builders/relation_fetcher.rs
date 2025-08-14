use crate::types::{EntityRegistry, RelationFetcher};
use crate::HasRelationMetadata;
use sea_orm::ConnectionTrait;

/// SeaORM-specific relation fetcher implementation
pub struct SeaOrmRelationFetcher<R: EntityRegistry<C>, C: ConnectionTrait> {
    pub entity_registry: R,
    pub _phantom: std::marker::PhantomData<C>,
}

impl<C: ConnectionTrait, ModelWithRelations, R: EntityRegistry<C>>
    RelationFetcher<C, ModelWithRelations> for SeaOrmRelationFetcher<R, C>
where
    ModelWithRelations: HasRelationMetadata<ModelWithRelations> + Send + 'static,
    R: Send + Sync,
    C: Send + Sync,
{
    fn fetch_relation_for_model<'a>(
        &'a self,
        conn: &'a C,
        model_with_relations: &'a mut ModelWithRelations,
        relation_name: &'a str,
        _filters: &'a [crate::types::Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
        let descriptor = match ModelWithRelations::get_relation_descriptor(relation_name) {
            Some(d) => d,
            None => {
                let fut = async move {
                    Err(sea_orm::DbErr::Custom(format!(
                        "Relation '{}' not found",
                        relation_name
                    )))
                };
                return Box::pin(fut);
            }
        };

        let type_name = std::any::type_name::<ModelWithRelations>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();

        let fut = async move {
            if let Some(fetcher) = self.entity_registry.get_fetcher(&fetcher_entity_name) {
                let result = fetcher
                    .fetch_by_foreign_key(
                        conn,
                        (descriptor.get_foreign_key)(model_with_relations),
                        descriptor.foreign_key_column,
                        &fetcher_entity_name,
                        relation_name,
                        &crate::types::RelationFilter {
                            relation: "",
                            filters: vec![],
                            nested_select_aliases: None,
                            nested_includes: vec![],
                            take: None,
                            skip: None,
                            order_by: vec![],
                            cursor_id: None,
                        },
                    )
                    .await?;
                (descriptor.set_field)(model_with_relations, result);
                Ok(())
            } else {
                Err(sea_orm::DbErr::Custom(format!(
                    "No fetcher found for entity: {}",
                    fetcher_entity_name
                )))
            }
        };
        Box::pin(fut)
    }
}

