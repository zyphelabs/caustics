use std::env;
use std::path::Path;
use std::fs;
use walkdir::WalkDir;
use syn::{parse_file, Item, ItemMod, ItemStruct, Attribute, Meta};
use quote::quote;

fn main() {
    // Tell cargo to rerun this if any source file changes
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=tests/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("caustics_client.rs");

    let mut entities = Vec::new();

    // Scan src/ and tests/ directories for Rust files
    for entry in WalkDir::new("src").into_iter().chain(WalkDir::new("tests").into_iter()) {
        let entry = entry.unwrap();
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap();
            let file = parse_file(&content).unwrap();

            // Process items in the file
            for item in file.items {
                if let Item::Struct(struct_item) = item {
                    // Check if struct has #[derive(Caustics)]
                    if has_caustics_derive(&struct_item.attrs) {
                        let name = struct_item.ident.to_string();
                        let module_path = get_module_path(entry.path());
                        entities.push((name, module_path));
                    }
                }
            }
        }
    }

    // Generate client code
    let client_code = generate_client_code(&entities);
    fs::write(out_path, client_code).unwrap();
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

fn get_module_path(file_path: &Path) -> String {
    let mut path = file_path.to_string_lossy().to_string();
    path = path.replace("src/", "").replace("tests/", "");
    path = path.replace(".rs", "");
    path.replace("/", "::")
}

fn generate_client_code(entities: &[(String, String)]) -> String {
    let entity_methods: Vec<_> = entities.iter().map(|(name, module_path)| {
        let method_name = name.to_lowercase();
        let entity_client = format!("{}Client", name);
        let module_path = module_path.replace("::", "::");
        
        quote! {
            pub fn #method_name(&self) -> #module_path::#entity_client {
                #module_path::#entity_client::new(self.db.clone())
            }
        }
    }).collect();

    let client_code = quote! {
        use sea_orm::{DatabaseConnection, Condition, QueryBuilder, Error};
        use std::sync::Arc;

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
    };

    client_code.to_string()
} 