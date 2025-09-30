use heck::ToSnakeCase;
use syn::DeriveInput;

/// Extract table name from entity attributes
pub fn extract_table_name(model_ast: &DeriveInput) -> String {
    for attr in &model_ast.attrs {
        if let syn::Meta::List(meta) = &attr.meta {
            if meta.path.is_ident("sea_orm") {
                if let Ok(nested) = meta.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let syn::Meta::NameValue(nv) = &meta {
                            if nv.path.is_ident("table_name") {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit),
                                    ..
                                }) = &nv.value
                                {
                                    return lit.value();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Default to snake_case of the struct name
    model_ast.ident.to_string().to_snake_case()
}
