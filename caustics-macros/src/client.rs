use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use crate::ENTITIES;

pub fn generate_client() -> TokenStream {
    let entity_names: Vec<String> = ENTITIES.lock().unwrap().iter().cloned().collect();
    let entity_methods: Vec<_> = entity_names.iter().map(|entity_name| {
        let method_name = format_ident!("{}", entity_name.to_lowercase());
        let entity_client = format_ident!("{}Client", entity_name);
        
        quote! {
            pub fn #method_name(&self) -> #entity_client {
                #entity_client::new(self.db.clone())
            }
        }
    }).collect();

    quote! {
        use sea_orm::DatabaseConnection;
        use std::sync::Arc;

        #[derive(Copy, Clone, Debug)]
        pub enum SortOrder {
            Asc,
            Desc,
        }

        // Private implementation to use SortOrder variants
        impl SortOrder {
            const _ASC: SortOrder = SortOrder::Asc;
            const _DESC: SortOrder = SortOrder::Desc;
        }

        pub struct CausticsClient {
            db: Arc<DatabaseConnection>,
        }

        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db: Arc::new(db) }
            }

            pub fn db(&self) -> &DatabaseConnection {
                &self.db
            }

            #(#entity_methods)*
        }
    }
} 