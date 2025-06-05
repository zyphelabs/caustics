use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, DataEnum, Variant};

pub fn generate_relation(ast: DeriveInput) -> TokenStream {
    let enum_ident = &ast.ident;
    
    // Extract variants from the enum
    let variants = match &ast.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        _ => panic!("Expected an enum"),
    };

    // Generate helper methods for each variant
    let helper_methods = generate_helper_methods(variants);

    // Generate the expanded code
    let expanded = quote! {
        impl #enum_ident {
            #helper_methods
        }
    };

    expanded
}

fn generate_helper_methods(variants: &syn::punctuated::Punctuated<Variant, syn::Token![,]>) -> TokenStream {
    let methods = variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let variant_name = variant_ident.to_string();
        let method_name = format_ident!("{}", variant_name.to_lowercase());
        
        quote! {
            pub fn #method_name() -> sea_orm::RelationDef {
                Self::#variant_ident.def()
            }
        }
    });

    quote! {
        #(#methods)*
    }
} 