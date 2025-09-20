use quote::{format_ident, quote};
use std::env;
use std::fs;
use std::path::Path;
use syn::{parse_file, Attribute, Item, Meta};
use walkdir::WalkDir;

fn main() {
    // Main client (for src/)
    generate_client_for_dir("src", "caustics_client.rs");

    // Test client (for src/ and tests/) - also generate per-namespace files
    generate_client_for_dir_multi(&["src", "tests"], "caustics_client_test.rs");
    generate_per_namespace_files(&["src", "tests"]);
}

fn generate_client_for_dir(dir: &str, out_file: &str) {
    println!("cargo:rerun-if-changed={}/", dir);

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join(out_file);

    let mut entities = Vec::new();

    for entry in WalkDir::new(dir) {
        let entry = entry.unwrap();
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = match fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let file = match parse_file(&content) {
                Ok(f) => f,
                Err(_) => continue,
            };

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
                let content = match fs::read_to_string(entry.path()) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let file = match parse_file(&content) {
                    Ok(f) => f,
                    Err(_) => continue,
                };

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

fn generate_per_namespace_files(dirs: &[&str]) {
    for dir in dirs {
        println!("cargo:rerun-if-changed={}/", dir);
    }

    let out_dir = env::var("OUT_DIR").unwrap();

    // Group entities by namespace
    let mut namespace_entities: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();

    for dir in dirs {
        for entry in WalkDir::new(dir) {
            let entry = entry.unwrap();
            if entry.path().extension().map_or(false, |ext| ext == "rs") {
                let content = match fs::read_to_string(entry.path()) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let file = match parse_file(&content) {
                    Ok(f) => f,
                    Err(_) => continue,
                };

                for item in file.items {
                    if let Item::Mod(module) = &item {
                        let module_name = module.ident.to_string();
                        if let Some((_, items)) = &module.content {
                            let namespace = extract_namespace_from_attrs(&module.attrs);
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
                                            namespace_entities
                                                .entry(namespace.clone())
                                                .or_insert_with(Vec::new)
                                                .push((entity_name, module_path));
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
                                let entities = namespace_entities
                                    .entry(namespace.clone())
                                    .or_insert_with(Vec::new);
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

    // Generate a separate file for each namespace
    for (namespace, entities) in namespace_entities {
        if !entities.is_empty() {
            // Check if we're in a test directory by looking at the current directory
            let is_test = dirs.iter().any(|dir| *dir == "tests");
            let out_file = if is_test {
                format!("caustics_client_{}_test.rs", namespace)
            } else {
                format!("caustics_client_{}.rs", namespace)
            };
            let out_path = Path::new(&out_dir).join(out_file);
            let client_code = generate_client_code(&entities, true);
            fs::write(out_path, client_code).unwrap();
        }
    }
}

fn extract_namespace_from_attrs(attrs: &[Attribute]) -> String {
    for attr in attrs {
        if attr.path().is_ident("caustics") {
            // Convert the attribute to a string and parse it manually
            let attr_str = quote! { #attr }.to_string();
            if attr_str.contains("namespace") {
                // Extract namespace from the attribute string
                if let Some(start) = attr_str.find("namespace = ") {
                    let after_namespace = &attr_str[start + 12..];
                    if let Some(end) = after_namespace.find('"') {
                        let after_quote = &after_namespace[end + 1..];
                        if let Some(end_quote) = after_quote.find('"') {
                            return after_quote[..end_quote].to_string();
                        }
                    }
                }
            }
        }
    }
    "default".to_string()
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
            let module_ident = format_ident!("{}", module_path);
            let module_tokens = if is_test {
                // In integration tests, this file is included inside the per-file module (e.g., blog_test),
                // so entity modules live under self::, not crate::
                quote! { self::#module_ident }
            } else {
                quote! { #module_ident }
            };
            quote! {
                pub fn #method_name(&self) -> #module_tokens::EntityClient<'_, DatabaseConnection> {
                    #module_tokens::EntityClient::new(&*self.db, self.database_backend)
                }
            }
        })
        .collect();

    let tx_entity_methods: Vec<_> = entities
        .iter()
        .map(|(name, module_path)| {
            let method_name = format_ident!("{}", name.to_lowercase());
            let module_ident = format_ident!("{}", module_path);
            let module_tokens = if is_test {
                quote! { self::#module_ident }
            } else {
                quote! { #module_ident }
            };
            quote! {
                pub fn #method_name(&self) -> #module_tokens::EntityClient<'_, DatabaseTransaction> {
                    #module_tokens::EntityClient::new(&*self.tx, self.database_backend)
                }
            }
        })
        .collect();

    // Generate the composite registry
    let registry_match_arms: Vec<_> = entities
        .iter()
        .map(|(name, module_path)| {
            let entity_name_lower = name.to_lowercase();
            let module_ident = format_ident!("{}", module_path);
            let module_tokens = if is_test {
                quote! { self::#module_ident }
            } else {
                quote! { #module_ident }
            };
            quote! {
                #entity_name_lower => Some(&#module_tokens::EntityFetcherImpl),
            }
        })
        .collect();

    // Determine import statements and prefixes based on is_test
    let (
        imports,
        registry_trait,
        fetcher_trait,
        batch_container,
        batch_query,
        batch_result,
        from_model,
        merge_into,
    ) = if is_test {
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
            quote! { caustics::MergeInto },
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
            quote! { crate::MergeInto },
        )
    };

    let entity_prelude_uses: Vec<_> = entities
        .iter()
        .map(|(_, module_path)| {
            let module_ident = format_ident!("{}", module_path);
            quote! { pub use self::#module_ident::prelude::*; }
        })
        .collect();

    let (prelude_use, prelude_block) = if is_test {
        (
            quote! { #[allow(unused_imports)] use self::prelude::*; },
            quote! { #[allow(ambiguous_glob_reexports)] pub mod prelude {} },
        )
    } else {
        (
            quote! { #[allow(unused_imports)] use self::prelude::*; },
            quote! {
                #[allow(ambiguous_glob_reexports)]
                pub mod prelude {
                    #(#entity_prelude_uses)*
                }
            },
        )
    };

    let raw_block = if is_test {
        quote! {
            // Use the library Raw type in tests to avoid duplicate types
            pub use caustics::Raw;
        }
    } else {
        quote! {
            // Raw SQL support (typed bindings and results)
            #[derive(Clone, Debug)]
            pub struct Raw {
                pub sql: String,
                pub params: Vec<sea_orm::Value>,
            }

            impl Raw {
                pub fn new<S: Into<String>>(sql: S, params: Vec<sea_orm::Value>) -> Self {
                    Self { sql: sql.into(), params }
                }
                pub fn push_param<T: Into<sea_orm::Value>>(&mut self, v: T) { self.params.push(v.into()); }
                pub fn with_params(mut self, mut extra: Vec<sea_orm::Value>) -> Self {
                    self.params.append(&mut extra);
                    self
                }
            }
        }
    };

    let client_code = quote! {
        #imports
        // Bring all extension traits into scope automatically (generated)
        #prelude_use
        // Arc is used directly to avoid conflicts with test imports

        #[allow(dead_code)]
        pub struct CausticsClient {
            db: std::sync::Arc<DatabaseConnection>,
            database_backend: sea_orm::DatabaseBackend,
        }

        #raw_block

        pub struct RawQuery<T> {
            db: std::sync::Arc<DatabaseConnection>,
            backend: sea_orm::DatabaseBackend,
            raw: Raw,
            _marker: std::marker::PhantomData<T>,
        }

        impl<T> RawQuery<T> {
            pub async fn exec(self) -> Result<Vec<T>, sea_orm::DbErr>
            where
                T: sea_orm::FromQueryResult + Send + Sync + 'static,
            {
                use sea_orm::{Statement, SelectorRaw, SelectModel};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let rows = SelectorRaw::<SelectModel<T>>::from_statement(stmt).all(self.db.as_ref()).await?;
                Ok(rows)
            }
        }

        pub struct RawExecute {
            db: std::sync::Arc<DatabaseConnection>,
            backend: sea_orm::DatabaseBackend,
            raw: Raw,
        }

        impl RawExecute {
            pub async fn exec(self) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
                use sea_orm::{Statement, ConnectionTrait};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let res = self.db.execute(stmt).await?;
                Ok(res)
            }
        }

        #[allow(dead_code)]
        pub struct TransactionCausticsClient {
            tx: std::sync::Arc<DatabaseTransaction>,
            database_backend: sea_orm::DatabaseBackend,
        }

        pub struct TransactionBuilder {
            db: std::sync::Arc<DatabaseConnection>,
            database_backend: sea_orm::DatabaseBackend,
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

        #[allow(dead_code)]
        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                use sea_orm::ConnectionTrait;
                let database_backend = db.get_database_backend();
                Self {
                    db: std::sync::Arc::new(db),
                    database_backend,
                }
            }

            pub fn db(&self) -> std::sync::Arc<DatabaseConnection> {
                self.db.clone()
            }

            pub fn database_backend(&self) -> sea_orm::DatabaseBackend {
                self.database_backend
            }

            pub fn _transaction(&self) -> TransactionBuilder {
                TransactionBuilder {
                    db: self.db.clone(),
                    database_backend: self.database_backend,
                }
            }

            // Prisma-style name (without $): alias to _transaction
            pub fn transaction(&self) -> TransactionBuilder {
                self._transaction()
            }

            // Raw SQL APIs
            pub fn _query_raw<T>(&self, raw: Raw) -> RawQuery<T> {
                RawQuery { db: self.db.clone(), backend: self.database_backend, raw, _marker: std::marker::PhantomData }
            }

            pub fn _execute_raw(&self, raw: Raw) -> RawExecute {
                RawExecute { db: self.db.clone(), backend: self.database_backend, raw }
            }

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
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Delete(result)
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

        // Crate-level prelude that re-exports all entity extension traits collected from entity modules
        #prelude_block

        #[allow(dead_code)]
        impl TransactionCausticsClient {
            pub fn new(tx: std::sync::Arc<DatabaseTransaction>, database_backend: sea_orm::DatabaseBackend) -> Self {
                Self { tx, database_backend }
            }

            #(#tx_entity_methods)*

            // Raw SQL APIs within a transaction
            pub fn _query_raw<T>(&self, raw: Raw) -> TxRawQuery<T> {
                TxRawQuery { tx: self.tx.clone(), backend: self.database_backend, raw, _marker: std::marker::PhantomData }
            }

            pub fn _execute_raw(&self, raw: Raw) -> TxRawExecute {
                TxRawExecute { tx: self.tx.clone(), backend: self.database_backend, raw }
            }
        }

        pub struct TxRawQuery<T> {
            tx: std::sync::Arc<DatabaseTransaction>,
            backend: sea_orm::DatabaseBackend,
            raw: Raw,
            _marker: std::marker::PhantomData<T>,
        }

        impl<T> TxRawQuery<T> {
            pub async fn exec(self) -> Result<Vec<T>, sea_orm::DbErr>
            where
                T: sea_orm::FromQueryResult + Send + Sync + 'static,
            {
                use sea_orm::{Statement, SelectorRaw, SelectModel};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let rows = SelectorRaw::<SelectModel<T>>::from_statement(stmt).all(self.tx.as_ref()).await?;
                Ok(rows)
            }
        }

        pub struct TxRawExecute {
            tx: std::sync::Arc<DatabaseTransaction>,
            backend: sea_orm::DatabaseBackend,
            raw: Raw,
        }

        impl TxRawExecute {
            pub async fn exec(self) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
                use sea_orm::{Statement, ConnectionTrait};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let res = self.tx.execute(stmt).await?;
                Ok(res)
            }
        }

        impl TransactionBuilder {
            pub async fn run<F, Fut, T>(&self, f: F) -> Result<T, sea_orm::DbErr>
            where
                F: FnOnce(TransactionCausticsClient) -> Fut,
                Fut: std::future::Future<Output = Result<T, sea_orm::DbErr>>,
            {
                let tx = self.db.begin().await?;
                let tx_arc = std::sync::Arc::new(tx);
                let tx_client = TransactionCausticsClient::new(tx_arc.clone(), self.database_backend);
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
