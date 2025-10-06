// Entity metadata structures and functions for dynamic foreign key field detection

#[derive(Debug, Clone)]
pub struct EntityMetadata {
    pub name: &'static str,
    pub table_name: &'static str,
    pub primary_key_field: &'static str,
    pub foreign_key_fields: &'static [&'static str],
    pub relations: &'static [EntityRelationMetadata],
    pub primary_key_type: &'static str,
    pub foreign_key_types: &'static [(&'static str, &'static str)],
}

#[derive(Debug, Clone)]
pub struct EntityRelationMetadata {
    pub name: &'static str,
    pub target_entity: &'static str,
    pub target_table_name: &'static str,
    pub foreign_key_field: Option<&'static str>,
    pub relation_kind: &'static str,
}

// Static entity metadata registry - empty by default
// This will be populated by the build script in user projects
static ENTITY_METADATA: &[EntityMetadata] = &[];

// Trait for entity metadata resolution
pub trait EntityMetadataProvider {
    fn get_entity_metadata(&self, entity_name: &str) -> Option<&'static EntityMetadata>;
}

// Helper function to get entity metadata with namespace-aware resolution
pub fn get_entity_metadata(entity_name: &str) -> Option<&'static EntityMetadata> {
    // Try exact match first
    if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == entity_name) {
        return Some(meta);
    }
    // Try namespace-aware resolution
    // 1. Try with namespace prefix (e.g., "blog::Post" -> "Post")
    else if let Some(colon_pos) = entity_name.rfind("::") {
        let name_without_namespace = &entity_name[colon_pos + 2..];
        if let Some(meta) = ENTITY_METADATA
            .iter()
            .find(|meta| meta.name == name_without_namespace)
        {
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
    if let Some(meta) = ENTITY_METADATA
        .iter()
        .find(|meta| meta.name == snake_to_pascal)
    {
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
