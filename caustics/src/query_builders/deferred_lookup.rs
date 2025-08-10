use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction};
use std::any::Any;

/// Internal structure for storing deferred foreign key lookups
pub struct DeferredLookup {
    pub unique_param: Box<dyn Any + Send>,
    pub assign: fn(&mut (dyn Any + 'static), i32),
    pub resolve_on_conn: Box<
        dyn for<'a> Fn(
                &'a DatabaseConnection,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send,
    >,
    pub resolve_on_txn: Box<
        dyn for<'a> Fn(
                &'a DatabaseTransaction,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send,
    >,
}

impl DeferredLookup {
    pub fn new(
        unique_param: Box<dyn Any + Send>,
        assign: fn(&mut (dyn Any + 'static), i32),
        resolve_on_conn: impl for<'a> Fn(
                &'a DatabaseConnection,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send
            + 'static,
        resolve_on_txn: impl for<'a> Fn(
                &'a DatabaseTransaction,
                &dyn Any,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>,
            > + Send
            + 'static,
    ) -> Self {
        Self {
            unique_param,
            assign,
            resolve_on_conn: Box::new(resolve_on_conn),
            resolve_on_txn: Box::new(resolve_on_txn),
        }
    }
}

pub trait DeferredResolveFor<C: ConnectionTrait> {
    fn resolve_for<'a>(
        &'a self,
        conn: &'a C,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>>;
}

impl DeferredResolveFor<DatabaseConnection> for DeferredLookup {
    fn resolve_for<'a>(
        &'a self,
        conn: &'a DatabaseConnection,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>> {
        (self.resolve_on_conn)(conn, &*self.unique_param)
    }
}

impl DeferredResolveFor<DatabaseTransaction> for DeferredLookup {
    fn resolve_for<'a>(
        &'a self,
        conn: &'a DatabaseTransaction,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i32, sea_orm::DbErr>> + Send + 'a>> {
        (self.resolve_on_txn)(conn, &*self.unique_param)
    }
}

