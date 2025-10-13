use quote::{format_ident, quote, ToTokens};
use heck::ToPascalCase;

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub is_optional: bool,
    pub is_primary_key: bool,
    pub is_created_at: bool,
    pub is_updated_at: bool,
    pub column_name: Option<String>,
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = format_ident!("{}", self.name);
        let ty = syn::parse_str::<syn::Type>(&self.ty).unwrap();
        tokens.extend(quote! { #name: #ty });
    }
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub name: String,
    pub target: syn::Path,
    pub kind: RelationKind,
    
    // Legacy single field support (for backward compatibility)
    pub foreign_key_field: Option<String>,
    pub foreign_key_type: Option<syn::Type>,
    pub foreign_key_column: Option<String>,
    pub primary_key_field: Option<String>,
    
    // NEW: Composite foreign key fields with their types
    pub foreign_key_fields: Vec<String>,
    pub foreign_key_types: Vec<syn::Type>,
    pub foreign_key_columns: Vec<String>,
    
    // Target primary key fields
    pub target_primary_key_fields: Vec<String>,
    pub target_primary_key_columns: Vec<String>,
    
    pub target_unique_param: Option<syn::Path>,
    pub is_nullable: bool,
    pub target_entity_name: Option<String>,
    pub current_table_name: Option<String>,
    
    // NEW: Composite relation metadata
    pub is_composite: bool,
    pub composite_key_mapping: Vec<(String, String)>, // (from_field, to_field)
    
    // Custom field name for the relation
    pub custom_field_name: Option<String>,
    
    // For has_one relations: whether the target entity's foreign key is optional
    pub target_fk_is_optional: Option<bool>,
}

impl Relation {
    /// Get the foreign key name by concatenating all foreign key fields in PascalCase joined by "And"
    pub fn get_fk_name(&self) -> String {
        if self.foreign_key_fields.is_empty() {
            panic!("Foreign key field not specified for relation '{}'", self.name);
        }
        
        if self.foreign_key_fields.len() == 1 {
            // Single foreign key - just return the PascalCase version
            self.foreign_key_fields[0].to_pascal_case()
        } else {
            // Composite foreign key - join all fields with "And"
            self.foreign_key_fields
                .iter()
                .map(|field| field.to_pascal_case())
                .collect::<Vec<_>>()
                .join("And")
        }
    }

    /// Get the first foreign key field name in PascalCase (for Column enum access)
    pub fn get_first_fk_column_name(&self) -> String {
        if !self.foreign_key_fields.is_empty() {
            self.foreign_key_fields[0].clone()  // Return the field name as-is (already snake_case)
        } else if let Some(fk_field) = &self.foreign_key_field {
            fk_field.clone()
        } else {
            panic!("No foreign key field specified for relation '{}'", self.name);
        }
    }

    /// Get the primary key name by concatenating all target primary key fields in PascalCase joined by "And"
    pub fn get_pk_name(&self) -> String {
        if self.target_primary_key_fields.is_empty() {
            panic!("Target primary key field not specified for relation '{}'", self.name);
        }
        
        if self.target_primary_key_fields.len() == 1 {
            // Single primary key - just return the PascalCase version
            self.target_primary_key_fields[0].to_pascal_case()
        } else {
            // Composite primary key - join all fields with "And"
            self.target_primary_key_fields
                .iter()
                .map(|field| field.to_pascal_case())
                .collect::<Vec<_>>()
                .join("And")
        }
    }

    /// Get the field name for this relation, using custom name if provided, otherwise pluralizing for HasMany
    pub fn get_field_name(&self) -> String {
        use inflector::Inflector;
        
        if let Some(custom_name) = &self.custom_field_name {
            custom_name.clone()
        } else if matches!(self.kind, RelationKind::HasMany) {
            // For HasMany relations, pluralize the relation name
            self.name.to_snake_case().to_plural()
        } else {
            // For BelongsTo and HasOne relations, use the relation name as-is (singular)
            self.name.to_snake_case()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationKind {
    HasMany,
    BelongsTo,
    HasOne,
}
