use quote::{format_ident, quote};
use std::env;
use std::fs;
use std::path::Path;
use syn::{parse_file, Attribute, Item, Meta};
use walkdir::WalkDir;

fn main() {
    // Main client (for src/)
    generate_client_for_dir("src", "caustics_client.rs");

    // Test client (for src/ and tests/)
    generate_client_for_dir_multi(&["src", "tests"], "caustics_client_test.rs");
}

fn generate_client_for_dir(dir: &str, out_file: &str) {
    println!("cargo:rerun-if-changed={}/", dir);

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join(out_file);

    let mut entities = Vec::new();

    for entry in WalkDir::new(dir) {
        let entry = entry.unwrap();
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap();
            let file = parse_file(&content).unwrap();

            for item in file.items {
                if let Item::Mod(module) = &item {
                    let module_name = module.ident.to_string();
                    if let Some((_, items)) = &module.content {
                        let has_caustics_attr = has_caustics_attribute(&module.attrs);
                        let mut model_found = false;
                        let mut relation_found = false;

                        for item in items {
                            if let Item::Struct(struct_item) = item {
                                if struct_item.ident == "Model" {
                                    model_found = true;
                                    if has_caustics_attr || has_caustics_derive(&struct_item.attrs)
                                    {
                                        let entity_name = to_pascal_case(&module_name);
                                        let module_path = module_name.clone();
                                        entities.push((entity_name, module_path));
                                    }
                                }
                            } else if let Item::Enum(enum_item) = item {
                                if enum_item.ident == "Relation" {
                                    relation_found = true;
                                }
                            }
                        }

                        // If we found both Model and Relation in a caustics module, ensure the entity is added
                        if has_caustics_attr && model_found && relation_found {
                            let entity_name = to_pascal_case(&module_name);
                            let module_path = module_name.clone();
                            if !entities.iter().any(|(name, _)| name == &entity_name) {
                                entities.push((entity_name, module_path));
                            }
                        }
                    }
                }
            }
        }
    }

    let client_code = generate_client_code(&entities, false);
    fs::write(out_path, client_code).unwrap();
}

fn generate_client_for_dir_multi(dirs: &[&str], out_file: &str) {
    for dir in dirs {
        println!("cargo:rerun-if-changed={}/", dir);
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join(out_file);

    let mut entities = Vec::new();

    for dir in dirs {
        for entry in WalkDir::new(dir) {
            let entry = entry.unwrap();
            if entry.path().extension().map_or(false, |ext| ext == "rs") {
                let content = fs::read_to_string(entry.path()).unwrap();
                let file = parse_file(&content).unwrap();

                for item in file.items {
                    if let Item::Mod(module) = &item {
                        let module_name = module.ident.to_string();
                        if let Some((_, items)) = &module.content {
                            let has_caustics_attr = has_caustics_attribute(&module.attrs);
                            let mut model_found = false;
                            let mut relation_found = false;

                            for item in items {
                                if let Item::Struct(struct_item) = item {
                                    if struct_item.ident == "Model" {
                                        model_found = true;
                                        if has_caustics_attr
                                            || has_caustics_derive(&struct_item.attrs)
                                        {
                                            let entity_name = to_pascal_case(&module_name);
                                            let module_path = module_name.clone();
                                            entities.push((entity_name, module_path));
                                        }
                                    }
                                } else if let Item::Enum(enum_item) = item {
                                    if enum_item.ident == "Relation" {
                                        relation_found = true;
                                    }
                                }
                            }

                            // If we found both Model and Relation in a caustics module, ensure the entity is added
                            if has_caustics_attr && model_found && relation_found {
                                let entity_name = to_pascal_case(&module_name);
                                let module_path = module_name.clone();
                                if !entities.iter().any(|(name, _)| name == &entity_name) {
                                    entities.push((entity_name, module_path));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Use test-specific client code generation
    let client_code = generate_client_code(&entities, true);
    fs::write(out_path, client_code).unwrap();
}

fn has_caustics_attribute(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("caustics"))
}

fn has_caustics_derive(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            if let Meta::List(meta) = &attr.meta {
                meta.tokens.to_string().contains("Caustics")
            } else {
                false
            }
        } else {
            false
        }
    })
}

fn to_pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            out.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn generate_client_code(entities: &[(String, String)], is_test: bool) -> String {
    let entity_methods: Vec<_> = entities
        .iter()
        .map(|(name, module_path)| {
            let method_name = format_ident!("{}", name.to_lowercase());
            let module_path = format_ident!("{}", module_path);
            quote! {
                pub fn #method_name(&self) -> #module_path::EntityClient<'_, DatabaseConnection> {
                    #module_path::EntityClient::new(&*self.db)
                }
            }
        })
        .collect();

    let tx_entity_methods: Vec<_> = entities
        .iter()
        .map(|(name, module_path)| {
            let method_name = format_ident!("{}", name.to_lowercase());
            let module_path = format_ident!("{}", module_path);
            quote! {
                pub fn #method_name(&self) -> #module_path::EntityClient<'_, DatabaseTransaction> {
                    #module_path::EntityClient::new(&*self.tx)
                }
            }
        })
        .collect();

    // Generate the composite registry
    let registry_match_arms: Vec<_> = entities
        .iter()
        .map(|(name, module_path)| {
            let entity_name_lower = name.to_lowercase();
            let module_path = format_ident!("{}", module_path);
            quote! {
                #entity_name_lower => Some(&#module_path::EntityFetcherImpl),
            }
        })
        .collect();

    // Determine import statements and prefixes based on is_test
    let (imports, registry_trait, fetcher_trait, batch_container, batch_query, batch_result, from_model, merge_into) = if is_test {
        (
            quote! {
                use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
                use caustics::{EntityRegistry, EntityFetcher};
            },
            quote! { EntityRegistry<C> },
            quote! { EntityFetcher },
            quote! { caustics::BatchContainer },
            quote! { caustics::BatchQuery },
            quote! { caustics::BatchResult },
            quote! { caustics::FromModel },
            quote! { caustics::MergeInto }
        )
    } else {
        (
            quote! {
                use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
            },
            quote! { crate::EntityRegistry<C> },
            quote! { crate::EntityFetcher },
            quote! { crate::BatchContainer },
            quote! { crate::BatchQuery },
            quote! { crate::BatchResult },
            quote! { crate::FromModel },
            quote! { crate::MergeInto }
        )
    };

    let client_code = quote! {
        #imports
        // Arc is used directly to avoid conflicts with test imports

        pub struct CausticsClient {
            db: std::sync::Arc<DatabaseConnection>,
        }

        #[allow(dead_code)]
        pub struct TransactionCausticsClient {
            tx: std::sync::Arc<DatabaseTransaction>,
        }

        pub struct TransactionBuilder {
            db: std::sync::Arc<DatabaseConnection>,
        }

        // Composite Entity Registry for relation fetching
        pub struct CompositeEntityRegistry;

        impl<C: sea_orm::ConnectionTrait> #registry_trait for CompositeEntityRegistry {
            fn get_fetcher(&self, entity_name: &str) -> Option<&dyn #fetcher_trait<C>> {
                match entity_name {
                    #(#registry_match_arms)*
                    _ => None,
                }
            }
        }

        // Implement for reference so &REGISTRY works as a trait object
        impl<C: sea_orm::ConnectionTrait> #registry_trait for &'static CompositeEntityRegistry {
            fn get_fetcher(&self, entity_name: &str) -> Option<&dyn #fetcher_trait<C>> {
                (**self).get_fetcher(entity_name)
            }
        }

        // Use a static registry instance
        static REGISTRY: CompositeEntityRegistry = CompositeEntityRegistry;
        pub fn get_registry() -> &'static CompositeEntityRegistry {
            &REGISTRY
        }

        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db: std::sync::Arc::new(db) }
            }

            pub fn db(&self) -> std::sync::Arc<DatabaseConnection> {
                self.db.clone()
            }

            pub fn _transaction(&self) -> TransactionBuilder {
                TransactionBuilder {
                    db: self.db.clone(),
                }
            }

            /// Execute multiple queries in a single transaction with fail-fast behavior
            pub async fn _batch<'a, Entity, ActiveModel, ModelWithRelations, T, Container>(
                &self,
                queries: Container,
            ) -> Result<Container::ReturnType, sea_orm::DbErr>
            where
                Entity: sea_orm::EntityTrait,
                ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
                ModelWithRelations: #from_model<<Entity as sea_orm::EntityTrait>::Model>,
                T: #merge_into<ActiveModel>,
                <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
                Container: #batch_container<'a, sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, T>,
            {
                let txn = self.db.begin().await?;
                let batch_queries = Container::into_queries(queries);
                let mut results = Vec::with_capacity(batch_queries.len());

                for query in batch_queries {
                    let res = match query {
                        #batch_query::Insert(q) => {
                            // For Insert, use exec_in_txn to use the transaction
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Insert(result)
                        }
                        #batch_query::Update(q) => {
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Update(result)
                        }
                        #batch_query::Delete(q) => {
                            q.exec_in_txn(&txn).await?;
                            #batch_result::Delete(())
                        }
                        #batch_query::Upsert(q) => {
                            // For Upsert, use exec_in_txn to use the transaction
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Upsert(result)
                        }
                    };
                    results.push(res);
                }

                txn.commit().await?;
                Ok(Container::from_results(results))
            }

            #(#entity_methods)*
        }

        impl TransactionCausticsClient {
            pub fn new(tx: std::sync::Arc<DatabaseTransaction>) -> Self {
                Self { tx }
            }

            #(#tx_entity_methods)*
        }

        impl TransactionBuilder {
            pub async fn run<F, Fut, T>(&self, f: F) -> Result<T, sea_orm::DbErr>
            where
                F: FnOnce(TransactionCausticsClient) -> Fut,
                Fut: std::future::Future<Output = Result<T, sea_orm::DbErr>>,
            {
                let tx = self.db.begin().await?;
                let tx_arc = std::sync::Arc::new(tx);
                let tx_client = TransactionCausticsClient::new(tx_arc.clone());
                let result = f(tx_client).await;
                let tx = std::sync::Arc::try_unwrap(tx_arc).expect("Transaction Arc should be unique");
                match result {
                    Ok(val) => {
                        tx.commit().await?;
                        Ok(val)
                    }
                    Err(e) => {
                        tx.rollback().await?;
                        Err(e)
                    }
                }
            }
        }
    };

    client_code.to_string()
}
