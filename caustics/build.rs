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

    let client_code = generate_client_code(&entities);
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

    let client_code = generate_client_code(&entities);
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

fn generate_client_code(entities: &[(String, String)]) -> String {
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

    let client_code = quote! {
        use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
        use caustics::{QueryError, SortOrder};

        pub struct CausticsClient {
            db: std::sync::Arc<DatabaseConnection>,
        }

        pub struct TransactionCausticsClient {
            tx: std::sync::Arc<DatabaseTransaction>,
        }

        pub struct TransactionBuilder {
            db: std::sync::Arc<DatabaseConnection>,
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
                let tx_arc = Arc::new(tx);
                let tx_client = TransactionCausticsClient::new(tx_arc.clone());
                let result = f(tx_client).await;
                let tx = Arc::try_unwrap(tx_arc).expect("Transaction Arc should be unique");
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
