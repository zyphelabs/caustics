mod code_gen;
mod relation_extraction;
mod relation_submodules;
mod table_name;
mod types;

pub use code_gen::generate_entity;
pub use relation_extraction::extract_relations;
pub use relation_submodules::generate_relation_submodules;
pub use types::{Relation, RelationKind};
