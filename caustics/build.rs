use quote::{format_ident, quote, ToTokens};
use std::env;
use std::fs;
use std::path::Path;
use syn::{parse_file, Attribute, Item, Meta};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct EntityMetadata {
    name: String,
    primary_key_field: String,
    foreign_key_fields: Vec<String>,
    relations: Vec<RelationMetadata>,
}

#[derive(Debug, Clone)]
struct RelationMetadata {
    name: String,
    target_entity: String,
    foreign_key_field: Option<String>,
    relation_kind: String, // "HasMany" or "BelongsTo"
}

fn extract_entity_metadata(file_path: &str, entity_name: &str, module_path: &str) -> Option<EntityMetadata> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_e) => {
            return None;
        }
    };
    
    let file = match parse_file(&content) {
        Ok(f) => f,
        Err(_) => return None,
    };
    
    let mut foreign_key_fields = Vec::new();
    let mut relations = Vec::new();
    let mut primary_key_field = None; // No default fallback - must be detected
    
    // Extract foreign key fields from Model struct (look inside modules)
    for item in &file.items {
        if let Item::Mod(module) = item {
            // Only process the module that matches our entity
            if module.ident.to_string() != *module_path {
                continue;
            }
            if let Some((_, items)) = &module.content {
                for module_item in items {
                    if let Item::Struct(struct_item) = module_item {
                        if struct_item.ident == "Model" {
                            if let syn::Fields::Named(fields) = &struct_item.fields {
                                for field in &fields.named {
                                    if let Some(field_name) = field.ident.as_ref() {
                                        let field_name_str = field_name.to_string();
                                        // Check if field ends with _id (foreign key pattern)
                                        if field_name_str.ends_with("_id") {
                                            foreign_key_fields.push(field_name_str.clone());
                                        }
                                        // Check if field is marked as primary key
                                        for attr in &field.attrs {
                                            if attr.path().is_ident("sea_orm") {
                                                // Check if the attribute contains "primary_key"
                                                if attr.meta.to_token_stream().to_string().contains("primary_key") {
                                                    primary_key_field = Some(field_name_str.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Extract relations from Relation enum (look inside modules)
    for item in &file.items {
        if let Item::Mod(module) = item {
            // Only process the module that matches our entity
            if module.ident.to_string() != *module_path {
                continue;
            }
            if let Some((_, items)) = &module.content {
                for module_item in items {
                    if let Item::Enum(enum_item) = module_item {
                        if enum_item.ident == "Relation" {
                for variant in &enum_item.variants {
                    let relation_name = variant.ident.to_string();
                    
                    // Parse the relation attributes to extract metadata
                    let mut target_entity = String::new();
                    let mut foreign_key_field = None;
                    let mut relation_kind = String::new();
                    
                    for attr in &variant.attrs {
                        if attr.path().is_ident("caustics") {
                            if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                                match meta {
                                    syn::Meta::NameValue(nv) => {
                                        if nv.path.is_ident("target") {
                                            if let syn::Expr::Path(expr_path) = &nv.value {
                                                if let Some(segment) = expr_path.path.segments.last() {
                                                    target_entity = segment.ident.to_string();
                                                }
                                            }
                                        } else if nv.path.is_ident("foreign_key") {
                                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                                    foreign_key_field = Some(lit_str.value());
                                                }
                                            }
                                        }
                                    }
                                    syn::Meta::List(meta_list) => {
                                        if meta_list.path.is_ident("caustics") {
                                            // Parse the tokens to extract relation kind
                                            let tokens_str = meta_list.tokens.to_string();
                                            if tokens_str.contains("HasMany") {
                                                relation_kind = "HasMany".to_string();
                                            } else if tokens_str.contains("BelongsTo") {
                                                relation_kind = "BelongsTo".to_string();
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    
                    if !target_entity.is_empty() {
                        relations.push(RelationMetadata {
                            name: relation_name,
                            target_entity,
                            foreign_key_field,
                            relation_kind,
                        });
                    }
                }
                        }
                    }
                }
            }
        }
    }
    
    Some(EntityMetadata {
        name: entity_name.to_string(),
        primary_key_field: primary_key_field.unwrap_or_else(|| {
            panic!("No primary key field found for entity '{}'. Please ensure at least one field is marked with #[primary_key] attribute.", entity_name)
        }),
        foreign_key_fields,
        relations,
    })
}

fn main() {
    // Debug: Print current working directory
    
    // Main client (for tests/) - include entities from tests/ only, metadata-only
    generate_client_for_dir("tests", "caustics_client.rs");

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
                                        let source_file = entry.path().to_string_lossy().to_string();
                                        entities.push((entity_name, module_path, source_file));
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
                            if !entities.iter().any(|(name, _, _)| name == &entity_name) {
                                let source_file = entry.path().to_string_lossy().to_string();
                                entities.push((entity_name, module_path, source_file));
                            }
                        }
                    }
                }
            }
        }
    }

    // Also include entities from tests directory for metadata
    for entry in WalkDir::new("tests") {
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
                                        let source_file = entry.path().to_string_lossy().to_string();
                                        entities.push((entity_name, module_path, source_file));
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
                            if !entities.iter().any(|(name, _, _)| name == &entity_name) {
                                let source_file = entry.path().to_string_lossy().to_string();
                                entities.push((entity_name, module_path, source_file));
                            }
                        }
                    }
                }
            }
        }
    }

    // Extract metadata for all entities
    let mut entities_metadata = Vec::new();
    for (entity_name, module_path, source_file) in &entities {
        let _file_path = format!("{}/{}.rs", dir, module_path);
        if let Some(metadata) = extract_entity_metadata(source_file, entity_name, module_path) {
            entities_metadata.push(metadata);
        }
    }
    
    // Generate only the entity metadata registry for the global client
    let client_code = generate_metadata_only_client(&entities_metadata);
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
                                            let source_file = entry.path().to_string_lossy().to_string();
                                entities.push((entity_name, module_path, source_file));
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
                                if !entities.iter().any(|(name, _, _)| name == &entity_name) {
                                    let source_file = entry.path().to_string_lossy().to_string();
                                entities.push((entity_name, module_path, source_file));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Extract metadata for all entities
    let mut entities_metadata = Vec::new();
    for (entity_name, module_path, source_file) in &entities {
        // Try to find the file in any of the directories
        let mut file_path = String::new();
        for dir in dirs {
            let test_path = format!("{}/{}.rs", dir, module_path);
            if std::path::Path::new(&test_path).exists() {
                file_path = test_path;
                break;
            }
        }
        
        if !file_path.is_empty() {
            if let Some(metadata) = extract_entity_metadata(source_file, entity_name, module_path) {
                entities_metadata.push(metadata);
            }
        }
    }
    
    // Convert entities to the format expected by generate_client_code
    let entities_for_codegen: Vec<(String, String)> = entities.iter()
        .map(|(name, path, _)| (name.clone(), path.clone()))
        .collect();
    // Determine if this is a test client based on the output file name
    let is_test = out_file.contains("_test");
    let client_code = generate_client_code(&entities_for_codegen, &entities_metadata, is_test);
    fs::write(out_path, client_code).unwrap();
}

fn generate_per_namespace_files(dirs: &[&str]) {
    for dir in dirs {
        println!("cargo:rerun-if-changed={}/", dir);
    }

    let out_dir = env::var("OUT_DIR").unwrap();

    // Group entities by namespace (entity_name, module_path, source_file)
    let mut namespace_entities: std::collections::HashMap<String, Vec<(String, String, String)>> =
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
                                            let source_file = entry.path().to_string_lossy().to_string();
                                            namespace_entities
                                                .entry(namespace.clone())
                                                .or_insert_with(Vec::new)
                                                .push((entity_name.clone(), module_path, source_file.clone()));
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
                                if !entities.iter().any(|(name, _, _)| name == &entity_name) {
                                    let source_file = entry.path().to_string_lossy().to_string();
                                entities.push((entity_name, module_path, source_file));
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
            // Extract metadata for entities in this namespace
            let mut entities_metadata = Vec::new();
            for (entity_name, module_path, source_file) in &entities {
                
                if let Some(metadata) = extract_entity_metadata(source_file, entity_name, module_path) {
                    entities_metadata.push(metadata);
                }
            }
            
            // Convert entities to the format expected by generate_client_code
            let entities_for_codegen: Vec<(String, String)> = entities.iter()
                .map(|(name, path, _)| (name.clone(), path.clone()))
                .collect();
            let client_code = generate_client_code(&entities_for_codegen, &entities_metadata, true);
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

fn generate_metadata_only_client(entities_metadata: &[EntityMetadata]) -> String {
    // Generate entity metadata
    let entity_metadata_items: Vec<_> = entities_metadata
        .iter()
        .map(|metadata| {
            let entity_name = &metadata.name;
            let foreign_key_fields = &metadata.foreign_key_fields;
            let relations = &metadata.relations;
            
            let fk_fields_lit = foreign_key_fields.iter().map(|f| quote! { #f }).collect::<Vec<_>>();
            let relations_lit = relations.iter().map(|rel| {
                let rel_name = &rel.name;
                let target_entity = &rel.target_entity;
                let fk_field = &rel.foreign_key_field;
                let relation_kind = &rel.relation_kind;
                quote! {
                    EntityRelationMetadata {
                        name: #rel_name,
                        target_entity: #target_entity,
                        foreign_key_field: #fk_field,
                        relation_kind: #relation_kind,
                    }
                }
            }).collect::<Vec<_>>();
            
            let primary_key_field_lit = syn::LitStr::new(&metadata.primary_key_field, proc_macro2::Span::call_site());
            quote! {
                EntityMetadata {
                    name: #entity_name,
                    primary_key_field: #primary_key_field_lit,
                    foreign_key_fields: &[#(#fk_fields_lit),*],
                    relations: &[#(#relations_lit),*],
                }
            }
        })
        .collect();
    
    let client_code = quote! {
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
        
        // Entity metadata for dynamic foreign key field detection
        #[derive(Debug, Clone)]
        pub struct EntityMetadata {
            pub name: &'static str,
            pub primary_key_field: &'static str,
            pub foreign_key_fields: &'static [&'static str],
            pub relations: &'static [EntityRelationMetadata],
        }
        
        #[derive(Debug, Clone)]
        pub struct EntityRelationMetadata {
            pub name: &'static str,
            pub target_entity: &'static str,
            pub foreign_key_field: Option<&'static str>,
            pub relation_kind: &'static str,
        }
        
        // Static entity metadata registry
        static ENTITY_METADATA: &[EntityMetadata] = &[
            #(#entity_metadata_items),*
        ];
        
        // Helper function to get entity metadata with namespace-aware resolution
        pub fn get_entity_metadata(entity_name: &str) -> Option<&'static EntityMetadata> {
            // Try exact match first
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == entity_name) {
                return Some(meta);
            }
            
            // Try namespace-aware resolution
            // 1. Try with namespace prefix (e.g., "blog::Post" -> "Post")
            if let Some(colon_pos) = entity_name.rfind("::") {
                let name_without_namespace = &entity_name[colon_pos + 2..];
                if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == name_without_namespace) {
                    return Some(meta);
                }
            }
            
            // 2. Try PascalCase variations
            let pascal_case = to_pascal_case(entity_name);
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == pascal_case) {
                return Some(meta);
            }
            
            // 3. Try snake_case to PascalCase conversion
            let snake_case = entity_name.to_lowercase();
            let snake_to_pascal = to_pascal_case(&snake_case);
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == snake_to_pascal) {
                return Some(meta);
            }
            
            None
        }
        
        // Helper function to convert to PascalCase
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
        
      
    };
    
    client_code.to_string()
}

fn generate_client_code(entities: &[(String, String)], entities_metadata: &[EntityMetadata], is_test: bool) -> String {
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

    let hooks_mod = if is_test {
        quote! { caustics::hooks }
    } else {
        quote! { crate::hooks }
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

    // Generate entity metadata
    let entity_metadata_items: Vec<_> = entities_metadata
        .iter()
        .map(|metadata| {
            let entity_name = &metadata.name;
            let foreign_key_fields = &metadata.foreign_key_fields;
            let relations = &metadata.relations;
            
            let fk_fields_lit = foreign_key_fields.iter().map(|f| quote! { #f }).collect::<Vec<_>>();
            let relations_lit = relations.iter().map(|rel| {
                let rel_name = &rel.name;
                let target_entity = &rel.target_entity;
                let fk_field = &rel.foreign_key_field;
                let relation_kind = &rel.relation_kind;
                quote! {
                    EntityRelationMetadata {
                        name: #rel_name,
                        target_entity: #target_entity,
                        foreign_key_field: #fk_field,
                        relation_kind: #relation_kind,
                    }
                }
            }).collect::<Vec<_>>();
            
            let primary_key_field_lit = syn::LitStr::new(&metadata.primary_key_field, proc_macro2::Span::call_site());
            quote! {
                EntityMetadata {
                    name: #entity_name,
                    primary_key_field: #primary_key_field_lit,
                    foreign_key_fields: &[#(#fk_fields_lit),*],
                    relations: &[#(#relations_lit),*],
                }
            }
        })
        .collect();
    
    let client_code = quote! {
        #imports
        // Bring all extension traits into scope automatically (generated)
        #prelude_use
        // Arc is used directly to avoid conflicts with test imports
        
        // Entity metadata for dynamic foreign key field detection
        #[derive(Debug, Clone)]
        pub struct EntityMetadata {
            pub name: &'static str,
            pub primary_key_field: &'static str,
            pub foreign_key_fields: &'static [&'static str],
            pub relations: &'static [EntityRelationMetadata],
        }
        
        #[derive(Debug, Clone)]
        pub struct EntityRelationMetadata {
            pub name: &'static str,
            pub target_entity: &'static str,
            pub foreign_key_field: Option<&'static str>,
            pub relation_kind: &'static str,
        }
        
        // Static entity metadata registry
        static ENTITY_METADATA: &[EntityMetadata] = &[
            #(#entity_metadata_items),*
        ];
        
        // Helper function to get entity metadata with namespace-aware resolution
        pub fn get_entity_metadata(entity_name: &str) -> Option<&'static EntityMetadata> {
            // Try exact match first
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == entity_name) {
                return Some(meta);
            }
            
            // Try namespace-aware resolution
            // 1. Try with namespace prefix (e.g., "blog::Post" -> "Post")
            if let Some(colon_pos) = entity_name.rfind("::") {
                let name_without_namespace = &entity_name[colon_pos + 2..];
                if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == name_without_namespace) {
                    return Some(meta);
                }
            }
            
            // 2. Try PascalCase variations
            let pascal_case = to_pascal_case(entity_name);
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == pascal_case) {
                return Some(meta);
            }
            
            // 3. Try snake_case to PascalCase conversion
            let snake_case = entity_name.to_lowercase();
            let snake_to_pascal = to_pascal_case(&snake_case);
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == snake_to_pascal) {
                return Some(meta);
            }
            
            None
        }
        
        // Helper function to convert to PascalCase
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

            // Transaction-scoped hook installer (overrides global while running in this thread)
            pub fn with_hook<F, Fut, T>(&self, hook: std::sync::Arc<dyn #hooks_mod::QueryHook>, f: F) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<T, sea_orm::DbErr>> + Send + '_>>
            where
                F: FnOnce(Self) -> Fut + Send + 'static,
                Fut: std::future::Future<Output = Result<T, sea_orm::DbErr>> + Send + 'static,
                T: Send + 'static,
            {
                Box::pin(async move {
                    #hooks_mod::set_thread_hook(Some(hook));
                    let _corr = #hooks_mod::set_new_correlation_id();
                    let res = f(TransactionCausticsClient::new(self.tx.clone(), self.database_backend)).await;
                    #hooks_mod::set_thread_hook(None);
                    #hooks_mod::set_thread_correlation_id(None);
                    res
                })
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
