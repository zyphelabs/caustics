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