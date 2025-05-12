use std::env;
use std::path::Path;
use std::fs;
use walkdir::WalkDir;
use syn::{parse_file, Item, Attribute, Meta};
use quote::{quote, format_ident};

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
                if let Item::Struct(struct_item) = item {
                    if has_caustics_derive(&struct_item.attrs) {
                        let name = struct_item.ident.to_string();
                        let module_path = get_module_path(entry.path());
                        entities.push((name, module_path));
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
                            for item in items {
                                if let Item::Struct(struct_item) = item {
                                    if struct_item.ident == "Model" && has_caustics_derive(&struct_item.attrs) {
                                        let entity_name = to_pascal_case(&module_name);
                                        let module_path = module_name.clone();
                                        entities.push((entity_name, module_path));
                                    }
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
    let entity_methods: Vec<_> = entities.iter().map(|(name, module_path)| {
        let method_name = format_ident!("{}", name.to_lowercase());
        let module_path = format_ident!("{}", module_path);
        quote! {
            pub fn #method_name(&self) -> #module_path::EntityClient {
                #module_path::EntityClient::new((*self.db).clone())
            }
        }
    }).collect();

    let client_code = quote! {
        use sea_orm::DatabaseConnection;
        use std::sync::Arc;

        #[derive(Copy, Clone, Debug)]
        #[allow(dead_code)]
        pub enum SortOrder {
            Asc,
            Desc,
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
    };

    client_code.to_string()
} 