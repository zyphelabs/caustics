use sea_orm::ConnectionTrait;
use std::any::Any;

/// Internal structure for storing deferred foreign key lookups
pub struct DeferredLookup<C: ConnectionTrait> {
    pub unique_param: Box<dyn Any + Send>,
    pub assign: fn(&mut (dyn Any + 'static), i32),
    pub entity_resolver: Box<
        dyn for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send,
    >,
    pub _phantom: std::marker::PhantomData<C>,
}

impl<C: ConnectionTrait> DeferredLookup<C> {
    pub fn new(
        unique_param: Box<dyn Any + Send>,
        assign: fn(&mut (dyn Any + 'static), i32),
        entity_resolver: impl for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send
            + 'static,
    ) -> Self {
        Self {
            unique_param,
            assign,
            entity_resolver: Box::new(entity_resolver),
            _phantom: std::marker::PhantomData,
        }
    }
}

