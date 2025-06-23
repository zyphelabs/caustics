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