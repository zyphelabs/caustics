use sea_orm::ConnectionTrait;

/// Trait to make DeferredLookup work with both regular connections and transactions
pub trait ConnectionLike: ConnectionTrait {}

impl<T: ConnectionTrait> ConnectionLike for T {}

