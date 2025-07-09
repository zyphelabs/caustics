use std::any::Any;

pub type QueryError = sea_orm::DbErr;

#[derive(Copy, Clone, Debug)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// Trait for converting a model to a model with relations
pub trait FromModel<M> {
    fn from_model(model: M) -> Self;
}

/// Trait for merging values into an ActiveModel
pub trait MergeInto<AM> {
    fn merge_into(&self, model: &mut AM);
}

/// Trait for relation filters that can be used with .with()
pub trait RelationFilterTrait: Clone {
    fn relation_name(&self) -> &'static str;
    fn filters(&self) -> &[Filter];
}

/// Generic filter structure that matches the generated Filter type
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub value: String,
}

/// Generic relation filter structure that matches the generated RelationFilter type
#[derive(Debug, Clone)]
pub struct RelationFilter {
    pub relation: &'static str,
    pub filters: Vec<Filter>,
}

impl RelationFilterTrait for RelationFilter {
    fn relation_name(&self) -> &'static str {
        self.relation
    }
    
    fn filters(&self) -> &[Filter] {
        &self.filters
    }
} 

/// Trait for dynamic relation fetching
pub trait RelationFetcher<C: sea_orm::ConnectionTrait, ModelWithRelations> {
    fn fetch_relation_for_model(
        &self,
        conn: &C,
        model_with_relations: &mut ModelWithRelations,
        relation_name: &str,
        filters: &[Filter],
    ) -> Result<(), sea_orm::DbErr>;
}

// Provide a default no-op implementation for all types
impl<C: sea_orm::ConnectionTrait, ModelWithRelations> RelationFetcher<C, ModelWithRelations> for () {
    fn fetch_relation_for_model(
        &self,
        _conn: &C,
        _model_with_relations: &mut ModelWithRelations,
        _relation_name: &str,
        _filters: &[Filter],
    ) -> Result<(), sea_orm::DbErr> {
        Ok(())
    }
} 

/// Descriptor for a relation, used for dynamic lookup
pub struct RelationDescriptor<ModelWithRelations> {
    pub name: &'static str,
    // Function to set the relation field on the model
    pub set_field: fn(&mut ModelWithRelations, Box<dyn Any + Send>),
    // Function to get the foreign key value from the model
    pub get_foreign_key: fn(&ModelWithRelations) -> Option<i32>,
    // The target entity name for the relation
    pub target_entity: &'static str,
    // The foreign key column name
    pub foreign_key_column: &'static str,
}

/// Trait for types that provide relation metadata
pub trait HasRelationMetadata<ModelWithRelations> {
    fn relation_descriptors() -> &'static [RelationDescriptor<ModelWithRelations>];
    fn get_relation_descriptor(name: &str) -> Option<&'static RelationDescriptor<ModelWithRelations>> {
        Self::relation_descriptors().iter().find(|desc| desc.name == name)
    }
} 

/// Trait for dynamic entity fetching without hardcoding
pub trait EntityFetcher<C: sea_orm::ConnectionTrait> {
    /// Fetch entities by foreign key value
    fn fetch_by_foreign_key<'a>(
        &'a self,
        conn: &'a C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &'a str,
        target_entity: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn Any + Send>, sea_orm::DbErr>> + Send + 'a>>;
}

/// Registry for mapping entity names to their fetchers
pub trait EntityRegistry<C: sea_orm::ConnectionTrait> {
    /// Get the fetcher for a given entity name
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn EntityFetcher<C>>;
}

/// Helper type for dynamic entity resolution
pub struct EntityResolver<C: sea_orm::ConnectionTrait> {
    pub registry: Box<dyn EntityRegistry<C> + Send + Sync>,
}

impl<C: sea_orm::ConnectionTrait> EntityResolver<C> {
    pub fn new(registry: Box<dyn EntityRegistry<C> + Send + Sync>) -> Self {
        Self { registry }
    }
    
    pub async fn resolve_entity(
        &self,
        conn: &C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &str,
        target_entity: &str,
    ) -> Result<Box<dyn Any + Send>, sea_orm::DbErr> {
        if let Some(fetcher) = self.registry.get_fetcher(target_entity) {
            fetcher.fetch_by_foreign_key(conn, foreign_key_value, foreign_key_column, target_entity).await
        } else {
            Err(sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", target_entity)))
        }
    }
} 