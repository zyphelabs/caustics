use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use walkdir::WalkDir;
use std::collections::HashSet;
use syn::{parse_file, Item, ItemMod, ItemStruct, Attribute, Meta};
use syn::parse::Parser;

fn main() {
    // Tell cargo to rerun this if any source file changes
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=tests/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("caustics_client.rs");
    let dest_path_test = Path::new(&out_dir).join("caustics_client_test.rs");

    let mut crate_entities = HashSet::new();
    let mut test_entities = HashSet::new();

    // Scan src/ and tests/
    for base in ["src", "tests"].iter() {
        for entry in WalkDir::new(base) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() && entry.path().extension().map(|e| e == "rs").unwrap_or(false) {
                let content = fs::read_to_string(entry.path()).unwrap();
                let ast = parse_file(&content).unwrap();
                let mut current_mods = Vec::new();
                let mut found_entities = Vec::new();
                process_items(&ast.items, &mut current_mods, &mut found_entities);
                for (fq_mod_path, struct_name) in found_entities {
                    if entry.path().to_string_lossy().starts_with("src/") {
                        crate_entities.insert((fq_mod_path, struct_name));
                    } else if entry.path().to_string_lossy().starts_with("tests/") {
                        test_entities.insert((fq_mod_path, struct_name));
                    }
                }
            }
        }
    }

    // Generate methods for each entity group
    let client_code = generate_client_code(&crate_entities);
    let client_code_test = generate_client_code(&test_entities);

    let mut f = File::create(&dest_path).unwrap();
    f.write_all(client_code.as_bytes()).unwrap();

    let mut f_test = File::create(&dest_path_test).unwrap();
    f_test.write_all(client_code_test.as_bytes()).unwrap();
}

fn process_items(items: &[Item], current_mods: &mut Vec<String>, found_entities: &mut Vec<(String, String)>) {
    for item in items {
        match item {
            Item::Mod(ItemMod { ident, content, .. }) => {
                current_mods.push(ident.to_string());
                if let Some((_, items)) = content {
                    process_items(items, current_mods, found_entities);
                }
                current_mods.pop();
            }
            Item::Struct(ItemStruct { ident, attrs, .. }) => {
                if has_caustics_derive(attrs) {
                    let fq_mod_path = current_mods.join("::");
                    found_entities.push((fq_mod_path, ident.to_string()));
                }
            }
            _ => {}
        }
    }
}

fn has_caustics_derive(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            if let Meta::List(meta) = &attr.meta {
                let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                if let Ok(nested) = parser.parse2(meta.tokens.clone()) {
                    nested.iter().any(|meta| {
                        if let Meta::Path(path) = meta {
                            path.is_ident("Caustics")
                        } else {
                            false
                        }
                    })
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    })
}

fn generate_client_code(entities: &HashSet<(String, String)>) -> String {
    let mut methods = String::new();
    for (fq_mod_path, _struct_name) in entities {
        let method_name = fq_mod_path.split("::").last().unwrap();
        let method = format!(
            "    pub fn {0}(&self) -> {1}::EntityClient {{\n        {1}::EntityClient::new(self.db.clone())\n    }}\n",
            method_name, fq_mod_path
        );
        methods.push_str(&method);
    }
    format!(
        "use sea_orm::DatabaseConnection;\n\npub struct CausticsClient {{\n    db: DatabaseConnection,\n}}\n\nimpl CausticsClient {{\n    pub fn new(db: DatabaseConnection) -> Self {{\n        Self {{ db }}\n    }}\n\n    pub fn db(&self) -> &DatabaseConnection {{\n        &self.db\n    }}\n\n{}}}\n",
        methods
    )
} 