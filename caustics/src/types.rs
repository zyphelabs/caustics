use std::any::Any;

pub type QueryError = sea_orm::DbErr;

// Import query builder types for batch operations
use crate::query_builders::{BatchQuery, BatchResult};

#[derive(Copy, Clone, Debug)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QueryMode {
    Default,
    Insensitive,
}

/// Trait for converting a model to a model with relations
pub trait FromModel<M> {
    fn from_model(model: M) -> Self;
}

/// Trait for merging values into an ActiveModel
pub trait MergeInto<AM> {
    fn merge_into(&self, model: &mut AM);
}

// Default implementation for unit type
impl<AM> MergeInto<AM> for () {
    fn merge_into(&self, _model: &mut AM) {
        // Unit type does nothing when merged
    }
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
        relation_name: &'a str,
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
        relation_name: &str,
    ) -> Result<Box<dyn Any + Send>, sea_orm::DbErr> {
        if let Some(fetcher) = self.registry.get_fetcher(target_entity) {
            fetcher.fetch_by_foreign_key(conn, foreign_key_value, foreign_key_column, target_entity, relation_name).await
        } else {
            Err(sea_orm::DbErr::Custom(format!("No fetcher found for entity: {}", target_entity)))
        }
    }
}

/// Trait for batch containers that can hold multiple queries (like Prisma Client Rust)
pub trait BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type ReturnType;
    fn into_queries(self) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>>;
    fn from_results(results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType;
}

/// Helper function to create batch queries
pub async fn batch<'a, C, Entity, ActiveModel, ModelWithRelations, T, Container>(
    queries: Container,
    conn: &'a C,
) -> Result<Container::ReturnType, sea_orm::DbErr>
where
    C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    Container: BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    let txn = conn.begin().await?;
    let batch_queries = Container::into_queries(queries);
    let mut results = Vec::with_capacity(batch_queries.len());

    for query in batch_queries {
        let res = match query {
            BatchQuery::Insert(q) => {
                let model = q.model;
                let result = model.insert(&txn).await.map(FromModel::from_model)?;
                BatchResult::Insert(result)
            }
            BatchQuery::Update(_) => {
                // For now, skip updates in batch mode
                return Err(sea_orm::DbErr::Custom("Update operations not supported in batch mode".to_string()));
            }
            BatchQuery::Delete(_) => {
                // For now, skip deletes in batch mode
                return Err(sea_orm::DbErr::Custom("Delete operations not supported in batch mode".to_string()));
            }
            BatchQuery::Upsert(_) => {
                // For now, skip upserts in batch mode
                return Err(sea_orm::DbErr::Custom("Upsert operations not supported in batch mode".to_string()));
            }
        };
        results.push(res);
    }

    txn.commit().await?;
    Ok(Container::from_results(results))
}

// Implementation for Vec
impl<'a, C, Entity, ActiveModel, ModelWithRelations, T> 
BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T> 
for Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>> 
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type ReturnType = Vec<BatchResult<ModelWithRelations>>;
    
    fn into_queries(self) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>> {
        self
    }
    
    fn from_results(results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        results
    }
}

// Implementation for tuples of CreateQueryBuilder (up to 4 elements for DatabaseConnection)
impl<'a, Entity, ActiveModel, ModelWithRelations>
BatchContainer<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>
for (crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,)
where
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    (): MergeInto<ActiveModel>,
{
    type ReturnType = (ModelWithRelations,);
    fn into_queries(self) -> Vec<BatchQuery<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>> {
        vec![BatchQuery::Insert(self.0)]
    }
    fn from_results(mut results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        let result1 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        (result1,)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
BatchContainer<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>
for (
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
)
where
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    (): MergeInto<ActiveModel>,
{
    type ReturnType = (ModelWithRelations, ModelWithRelations);
    fn into_queries(self) -> Vec<BatchQuery<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>> {
        vec![BatchQuery::Insert(self.0), BatchQuery::Insert(self.1)]
    }
    fn from_results(mut results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        let result1 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result2 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        (result1, result2)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
BatchContainer<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>
for (
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
)
where
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    (): MergeInto<ActiveModel>,
{
    type ReturnType = (ModelWithRelations, ModelWithRelations, ModelWithRelations);
    fn into_queries(self) -> Vec<BatchQuery<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>> {
        vec![BatchQuery::Insert(self.0), BatchQuery::Insert(self.1), BatchQuery::Insert(self.2)]
    }
    fn from_results(mut results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        let result1 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result2 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result3 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        (result1, result2, result3)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
BatchContainer<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>
for (
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
    crate::query_builders::CreateQueryBuilder<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations>,
)
where
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    (): MergeInto<ActiveModel>,
{
    type ReturnType = (ModelWithRelations, ModelWithRelations, ModelWithRelations, ModelWithRelations);
    fn into_queries(self) -> Vec<BatchQuery<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, ()>> {
        vec![BatchQuery::Insert(self.0), BatchQuery::Insert(self.1), BatchQuery::Insert(self.2), BatchQuery::Insert(self.3)]
    }
    fn from_results(mut results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        let result1 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result2 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result3 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        let result4 = match results.remove(0) { BatchResult::Insert(model) => model, _ => panic!("Expected Insert result") };
        (result1, result2, result3, result4)
    }
} 

/// Read filters for querying (PCR-compatible)
pub mod read_filters {
    use crate::QueryMode;
    
    #[derive(Debug, Clone)]
    pub enum StringFilter {
        Equals(String),
        In(Vec<String>),
        NotIn(Vec<String>),
        Lt(String),
        Lte(String),
        Gt(String),
        Gte(String),
        Contains(String),
        StartsWith(String),
        EndsWith(String),
        Not(Option<String>),
        Mode(QueryMode),
    }
    
    #[derive(Debug, Clone)]
    pub enum StringNullableFilter {
        Equals(Option<String>),
        In(Vec<String>),
        NotIn(Vec<String>),
        Lt(String),
        Lte(String),
        Gt(String),
        Gte(String),
        Contains(String),
        StartsWith(String),
        EndsWith(String),
        Not(Option<String>),
        Mode(QueryMode),
    }
    
    #[derive(Debug, Clone)]
    pub enum IntFilter {
        Equals(i32),
        In(Vec<i32>),
        NotIn(Vec<i32>),
        Lt(i32),
        Lte(i32),
        Gt(i32),
        Gte(i32),
        Not(Option<i32>),
    }
    
    #[derive(Debug, Clone)]
    pub enum IntNullableFilter {
        Equals(Option<i32>),
        In(Vec<i32>),
        NotIn(Vec<i32>),
        Lt(i32),
        Lte(i32),
        Gt(i32),
        Gte(i32),
        Not(Option<i32>),
    }
    
    #[derive(Debug, Clone)]
    pub enum DateTimeFilter {
        Equals(chrono::DateTime<chrono::FixedOffset>),
        In(Vec<chrono::DateTime<chrono::FixedOffset>>),
        NotIn(Vec<chrono::DateTime<chrono::FixedOffset>>),
        Lt(chrono::DateTime<chrono::FixedOffset>),
        Lte(chrono::DateTime<chrono::FixedOffset>),
        Gt(chrono::DateTime<chrono::FixedOffset>),
        Gte(chrono::DateTime<chrono::FixedOffset>),
        Not(Option<chrono::DateTime<chrono::FixedOffset>>),
    }
    
    #[derive(Debug, Clone)]
    pub enum DateTimeNullableFilter {
        Equals(Option<chrono::DateTime<chrono::FixedOffset>>),
        In(Vec<chrono::DateTime<chrono::FixedOffset>>),
        NotIn(Vec<chrono::DateTime<chrono::FixedOffset>>),
        Lt(chrono::DateTime<chrono::FixedOffset>),
        Lte(chrono::DateTime<chrono::FixedOffset>),
        Gt(chrono::DateTime<chrono::FixedOffset>),
        Gte(chrono::DateTime<chrono::FixedOffset>),
        Not(Option<chrono::DateTime<chrono::FixedOffset>>),
    }
    
    #[derive(Debug, Clone)]
    pub enum BoolFilter {
        Equals(bool),
        Not(Option<bool>),
    }
    
    #[derive(Debug, Clone)]
    pub enum JsonNullableFilter {
        Equals(Option<serde_json::Value>),
        Path(Vec<String>),
        StringContains(String),
        StringStartsWith(String),
        StringEndsWith(String),
        ArrayContains(Option<serde_json::Value>),
        ArrayStartsWith(Option<serde_json::Value>),
        ArrayEndsWith(Option<serde_json::Value>),
        Lt(serde_json::Value),
        Lte(serde_json::Value),
        Gt(serde_json::Value),
        Gte(serde_json::Value),
        Not(serde_json::Value),
    }
}

/// Write parameters for creating/updating (PCR-compatible)
pub mod write_params {
    #[derive(Debug, Clone)]
    pub enum StringParam {
        Set(String),
    }
    
    #[derive(Debug, Clone)]
    pub enum StringNullableParam {
        Set(Option<String>),
    }
    
    #[derive(Debug, Clone)]
    pub enum IntParam {
        Set(i32),
        Increment(i32),
        Decrement(i32),
        Multiply(i32),
        Divide(i32),
    }
    
    #[derive(Debug, Clone)]
    pub enum IntNullableParam {
        Set(Option<i32>),
        Increment(i32),
        Decrement(i32),
        Multiply(i32),
        Divide(i32),
    }
    
    #[derive(Debug, Clone)]
    pub enum DateTimeParam {
        Set(chrono::DateTime<chrono::FixedOffset>),
    }
    
    #[derive(Debug, Clone)]
    pub enum DateTimeNullableParam {
        Set(Option<chrono::DateTime<chrono::FixedOffset>>),
    }
    
    #[derive(Debug, Clone)]
    pub enum BoolParam {
        Set(bool),
    }
    
    #[derive(Debug, Clone)]
    pub enum JsonParam {
        Set(serde_json::Value),
    }
    
    #[derive(Debug, Clone)]
    pub enum JsonNullableParam {
        Set(Option<serde_json::Value>),
    }
}

// Integration layer for making read_filters work with FieldOp<T>
impl From<read_filters::StringFilter> for String {
    fn from(filter: read_filters::StringFilter) -> Self {
        match filter {
            read_filters::StringFilter::Equals(value) => value,
            read_filters::StringFilter::Contains(value) => value,
            read_filters::StringFilter::StartsWith(value) => value,
            read_filters::StringFilter::EndsWith(value) => value,
            read_filters::StringFilter::Lt(value) => value,
            read_filters::StringFilter::Lte(value) => value,
            read_filters::StringFilter::Gt(value) => value,
            read_filters::StringFilter::Gte(value) => value,
            _ => panic!("Unsupported filter operation for simple string conversion"),
        }
    }
}

impl From<read_filters::StringNullableFilter> for Option<String> {
    fn from(filter: read_filters::StringNullableFilter) -> Self {
        match filter {
            read_filters::StringNullableFilter::Equals(value) => value,
            read_filters::StringNullableFilter::Contains(value) => Some(value),
            read_filters::StringNullableFilter::StartsWith(value) => Some(value),
            read_filters::StringNullableFilter::EndsWith(value) => Some(value),
            read_filters::StringNullableFilter::Lt(value) => Some(value),
            read_filters::StringNullableFilter::Lte(value) => Some(value),
            read_filters::StringNullableFilter::Gt(value) => Some(value),
            read_filters::StringNullableFilter::Gte(value) => Some(value),
            _ => panic!("Unsupported filter operation for simple string conversion"),
        }
    }
}

impl From<read_filters::IntFilter> for i32 {
    fn from(filter: read_filters::IntFilter) -> Self {
        match filter {
            read_filters::IntFilter::Equals(value) => value,
            read_filters::IntFilter::Lt(value) => value,
            read_filters::IntFilter::Lte(value) => value,
            read_filters::IntFilter::Gt(value) => value,
            read_filters::IntFilter::Gte(value) => value,
            _ => panic!("Unsupported filter operation for simple int conversion"),
        }
    }
}

impl From<read_filters::IntNullableFilter> for Option<i32> {
    fn from(filter: read_filters::IntNullableFilter) -> Self {
        match filter {
            read_filters::IntNullableFilter::Equals(value) => value,
            read_filters::IntNullableFilter::Lt(value) => Some(value),
            read_filters::IntNullableFilter::Lte(value) => Some(value),
            read_filters::IntNullableFilter::Gt(value) => Some(value),
            read_filters::IntNullableFilter::Gte(value) => Some(value),
            _ => panic!("Unsupported filter operation for simple int conversion"),
        }
    }
}

impl From<read_filters::DateTimeFilter> for chrono::DateTime<chrono::FixedOffset> {
    fn from(filter: read_filters::DateTimeFilter) -> Self {
        match filter {
            read_filters::DateTimeFilter::Equals(value) => value,
            read_filters::DateTimeFilter::Lt(value) => value,
            read_filters::DateTimeFilter::Lte(value) => value,
            read_filters::DateTimeFilter::Gt(value) => value,
            read_filters::DateTimeFilter::Gte(value) => value,
            _ => panic!("Unsupported filter operation for simple datetime conversion"),
        }
    }
}

impl From<read_filters::DateTimeNullableFilter> for Option<chrono::DateTime<chrono::FixedOffset>> {
    fn from(filter: read_filters::DateTimeNullableFilter) -> Self {
        match filter {
            read_filters::DateTimeNullableFilter::Equals(value) => value,
            read_filters::DateTimeNullableFilter::Lt(value) => Some(value),
            read_filters::DateTimeNullableFilter::Lte(value) => Some(value),
            read_filters::DateTimeNullableFilter::Gt(value) => Some(value),
            read_filters::DateTimeNullableFilter::Gte(value) => Some(value),
            _ => panic!("Unsupported filter operation for simple datetime conversion"),
        }
    }
}

impl From<read_filters::BoolFilter> for bool {
    fn from(filter: read_filters::BoolFilter) -> Self {
        match filter {
            read_filters::BoolFilter::Equals(value) => value,
            _ => panic!("Unsupported filter operation for simple bool conversion"),
        }
    }
}

// Integration layer for making write_params work with SetParam system
impl From<write_params::StringParam> for String {
    fn from(param: write_params::StringParam) -> Self {
        match param {
            write_params::StringParam::Set(value) => value,
        }
    }
}

impl From<write_params::StringNullableParam> for Option<String> {
    fn from(param: write_params::StringNullableParam) -> Self {
        match param {
            write_params::StringNullableParam::Set(value) => value,
        }
    }
}

impl From<write_params::IntParam> for i32 {
    fn from(param: write_params::IntParam) -> Self {
        match param {
            write_params::IntParam::Set(value) => value,
            _ => panic!("Only Set operations supported for simple int conversion"),
        }
    }
}

impl From<write_params::IntNullableParam> for Option<i32> {
    fn from(param: write_params::IntNullableParam) -> Self {
        match param {
            write_params::IntNullableParam::Set(value) => value,
            _ => panic!("Only Set operations supported for simple int conversion"),
        }
    }
}

impl From<write_params::DateTimeParam> for chrono::DateTime<chrono::FixedOffset> {
    fn from(param: write_params::DateTimeParam) -> Self {
        match param {
            write_params::DateTimeParam::Set(value) => value,
        }
    }
}

impl From<write_params::DateTimeNullableParam> for Option<chrono::DateTime<chrono::FixedOffset>> {
    fn from(param: write_params::DateTimeNullableParam) -> Self {
        match param {
            write_params::DateTimeNullableParam::Set(value) => value,
        }
    }
}

impl From<write_params::BoolParam> for bool {
    fn from(param: write_params::BoolParam) -> Self {
        match param {
            write_params::BoolParam::Set(value) => value,
        }
    }
} 
