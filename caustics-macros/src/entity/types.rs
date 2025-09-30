use quote::{format_ident, quote, ToTokens};

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
    pub foreign_key_field: Option<String>,
    pub foreign_key_type: Option<syn::Type>,
    pub target_unique_param: Option<syn::Path>,
    pub is_nullable: bool,
    pub foreign_key_column: Option<String>,
    pub primary_key_field: Option<String>,
    pub target_entity_name: Option<String>, // Entity name extracted from "to" attribute
    pub current_table_name: Option<String>,
    pub target_table_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationKind {
    HasMany,
    BelongsTo,
}
