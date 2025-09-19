// Include generated code with composite registry
include!(concat!(env!("OUT_DIR"), "/caustics_client.rs"));

pub mod query_builders;
pub mod types;

pub use query_builders::*;
pub use types::*;

// Re-export DeferredLookup for use in macros
pub use query_builders::DeferredLookup;

// Re-export traits for use in generated code
pub use types::{EntityFetcher, EntityRegistry};
pub use types::ApplyNestedIncludes as ApplyNestedIncludes;

// Legacy Select! and select_typed! macros removed; use per-entity `entity::select!(...)` and builder `.select(...)`.

// Global typed selection macro that returns a SelectionSpec marker
// Global select_typed! macro no longer exposed
