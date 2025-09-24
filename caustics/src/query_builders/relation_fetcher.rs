use crate::types::{EntityRegistry, RelationFetcher};
use crate::HasRelationMetadata;
use sea_orm::ConnectionTrait;

/// SeaORM-specific relation fetcher implementation
pub struct SeaOrmRelationFetcher<R: EntityRegistry<C>, C: ConnectionTrait> {
    pub entity_registry: R,
    pub _phantom: std::marker::PhantomData<C>,
}

impl<C: ConnectionTrait, Selected, R: EntityRegistry<C>> RelationFetcher<C, Selected>
    for SeaOrmRelationFetcher<R, C>
where
    Selected: HasRelationMetadata<Selected> + Send + 'static,
    R: Send + Sync,
    C: Send + Sync,
{
    fn fetch_relation_for_model<'a>(
        &'a self,
        conn: &'a C,
        selected: &'a mut Selected,
        relation_name: &'a str,
        _filters: &'a [crate::types::Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>
    {
        let descriptor = match Selected::get_relation_descriptor(relation_name) {
            Some(d) => d,
            None => {
                let fut = async move {
                    Err(crate::types::CausticsError::RelationNotFound {
                        relation: relation_name.to_string(),
                    }
                    .into())
                };
                return Box::pin(fut);
            }
        };

        let type_name = std::any::type_name::<Selected>();
        let fetcher_entity_name = type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase();

        let fut = async move {
            if let Some(fetcher) = self.entity_registry.get_fetcher(&fetcher_entity_name) {
                let result = fetcher
                    .fetch_by_foreign_key(
                        conn,
                        (descriptor.get_foreign_key)(selected),
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
                            include_count: false,
                            distinct: false,
                        },
                    )
                    .await?;
                (descriptor.set_field)(selected, result);
                Ok(())
            } else {
                Err(crate::types::CausticsError::EntityFetcherMissing {
                    entity: fetcher_entity_name,
                }
                .into())
            }
        };
        Box::pin(fut)
    }
}
