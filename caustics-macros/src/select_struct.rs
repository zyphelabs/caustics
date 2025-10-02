use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::TokenTree;
use quote::{quote, ToTokens};
use syn::{
    braced, parse::Parse, parse::ParseStream, parse_macro_input, token::Brace, Ident, Token, Type,
};

// Define the 'from' token
syn::custom_keyword!(from);

/// Main entry point for the select_struct! macro
pub fn select_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SelectStructInput);

    match generate_select_struct(&input) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Parse the DSL input structure
#[derive(Debug)]
struct SelectStructInput {
    name: Ident,
    source_type: Option<Type>,
    fields: Vec<FieldDefinition>,
}

#[derive(Debug)]
struct FieldDefinition {
    name: Ident,
    field_type: FieldType,
}

#[derive(Debug)]
enum FieldType {
    Primitive(Type),
    Vec(Box<FieldType>),
    Option(Box<FieldType>),
    Nested(SelectStructInput),
}

impl Parse for SelectStructInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        // Always require explicit source type - no more inference
        input.parse::<from>()?;
        let source_type = input.parse::<Type>()?;

        let content;
        braced!(content in input);

        let fields = content
            .parse_terminated(FieldDefinition::parse, Token![,])?
            .into_iter()
            .collect();

        Ok(SelectStructInput {
            name,
            source_type: Some(source_type),
            fields,
        })
    }
}

impl Parse for FieldDefinition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let field_type = FieldType::parse(input)?;

        Ok(FieldDefinition { name, field_type })
    }
}

impl Parse for FieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) {
            let ident: Ident = input.parse()?;

            if ident == "Vec" {
                // Parse Vec<...> syntax
                input.parse::<Token![<]>()?;
                let inner_type = FieldType::parse(input)?;
                input.parse::<Token![>]>()?;
                Ok(FieldType::Vec(Box::new(inner_type)))
            } else if ident == "Option" {
                // Parse Option<...> syntax
                input.parse::<Token![<]>()?;
                let inner_type = FieldType::parse(input)?;
                input.parse::<Token![>]>()?;
                Ok(FieldType::Option(Box::new(inner_type)))
            } else if input.peek(from) {
                // This is a nested struct with explicit source type
                input.parse::<from>()?;
                let source_type = input.parse::<Type>()?;

                let content;
                braced!(content in input);
                let nested_fields = content
                    .parse_terminated(FieldDefinition::parse, Token![,])?
                    .into_iter()
                    .collect();

                Ok(FieldType::Nested(SelectStructInput {
                    name: ident,
                    source_type: Some(source_type),
                    fields: nested_fields,
                }))
            } else if input.peek(Brace) {
                // This is a nested struct without explicit source type (backward compatibility)
                let content;
                braced!(content in input);
                let nested_fields = content
                    .parse_terminated(FieldDefinition::parse, Token![,])?
                    .into_iter()
                    .collect();

                Ok(FieldType::Nested(SelectStructInput {
                    name: ident,
                    source_type: None, // Will use inference
                    fields: nested_fields,
                }))
            } else if input.peek(Token![<]) {
                // This is a generic type like DateTime<FixedOffset>
                // We need to parse the full type including generics
                let full_type = format!("{}", ident.to_token_stream());
                let mut depth = 0;
                let mut tokens = vec![ident.to_token_stream()];

                while !input.is_empty() {
                    if input.peek(Token![<]) {
                        depth += 1;
                        tokens.push(input.parse::<Token![<]>()?.to_token_stream());
                    } else if input.peek(Token![>]) {
                        depth -= 1;
                        tokens.push(input.parse::<Token![>]>()?.to_token_stream());
                        if depth == 0 {
                            break;
                        }
                    } else {
                        tokens.push(input.parse::<TokenTree>()?.to_token_stream());
                    }
                }

                let full_type_str = tokens
                    .into_iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join("");
                let ty = syn::parse_str::<syn::Type>(&full_type_str)?;
                Ok(FieldType::Primitive(ty))
            } else {
                // This is a primitive type
                let ty = syn::parse2(ident.to_token_stream())?;
                Ok(FieldType::Primitive(ty))
            }
        } else {
            // Try to parse as a general type
            let ty: Type = input.parse()?;
            Ok(FieldType::Primitive(ty))
        }
    }
}

/// Generate the complete select_struct implementation
fn generate_select_struct(input: &SelectStructInput) -> syn::Result<TokenStream2> {
    let mut all_structs = Vec::new();
    let mut all_from_impls = Vec::new();

    // Generate the main struct
    let main_struct = generate_struct(&input.name, &input.fields);
    all_structs.push(main_struct);

    // Generate nested structs
    generate_nested_structs(input, &mut all_structs);

    // Generate From implementations
    generate_from_implementations(input, &mut all_from_impls)?;

    Ok(quote! {
        #(#all_structs)*
        #(#all_from_impls)*
    })
}

/// Generate a struct definition
fn generate_struct(name: &Ident, fields: &[FieldDefinition]) -> TokenStream2 {
    let field_definitions = fields.iter().map(|field| {
        let field_name = &field.name;
        let field_type = match &field.field_type {
            FieldType::Primitive(ty) => quote! { #ty },
            FieldType::Vec(inner) => {
                let inner_type = extract_type_for_field(inner);
                quote! { Vec<#inner_type> }
            }
            FieldType::Option(inner) => {
                let inner_type = extract_type_for_field(inner);
                quote! { Option<#inner_type> }
            }
            FieldType::Nested(nested) => {
                let nested_name = &nested.name;
                quote! { #nested_name }
            }
        };

        quote! {
            pub #field_name: #field_type,
        }
    });

    quote! {
        #[derive(Debug, Clone)]
        pub struct #name {
            #(#field_definitions)*
        }
    }
}

/// Extract the type name for a field type
fn extract_type_for_field(field_type: &FieldType) -> TokenStream2 {
    match field_type {
        FieldType::Primitive(ty) => quote! { #ty },
        FieldType::Vec(inner) => {
            let inner_type = extract_type_for_field(inner);
            quote! { Vec<#inner_type> }
        }
        FieldType::Option(inner) => {
            let inner_type = extract_type_for_field(inner);
            quote! { Option<#inner_type> }
        }
        FieldType::Nested(nested) => {
            let nested_name = &nested.name;
            quote! { #nested_name }
        }
    }
}

/// Generate nested structs recursively
fn generate_nested_structs(input: &SelectStructInput, all_structs: &mut Vec<TokenStream2>) {
    for field in &input.fields {
        match &field.field_type {
            FieldType::Nested(nested) => {
                // Generate the nested struct
                let nested_struct = generate_struct(&nested.name, &nested.fields);
                all_structs.push(nested_struct);

                // Recursively generate deeper nested structs
                generate_nested_structs(nested, all_structs);
            }
            FieldType::Vec(inner) => {
                generate_nested_structs_from_field_type(inner, all_structs);
            }
            FieldType::Option(inner) => {
                generate_nested_structs_from_field_type(inner, all_structs);
            }
            _ => {}
        }
    }
}

/// Generate nested structs from a field type
fn generate_nested_structs_from_field_type(
    field_type: &FieldType,
    all_structs: &mut Vec<TokenStream2>,
) {
    match field_type {
        FieldType::Nested(nested) => {
            // Generate the nested struct
            let nested_struct = generate_struct(&nested.name, &nested.fields);
            all_structs.push(nested_struct);

            // Recursively generate deeper nested structs
            generate_nested_structs(nested, all_structs);
        }
        FieldType::Vec(inner) => {
            generate_nested_structs_from_field_type(inner, all_structs);
        }
        FieldType::Option(inner) => {
            generate_nested_structs_from_field_type(inner, all_structs);
        }
        _ => {}
    }
}

/// Generate From implementations for all structs
fn generate_from_implementations(
    input: &SelectStructInput,
    all_from_impls: &mut Vec<TokenStream2>,
) -> syn::Result<()> {
    // Generate From implementation for the main struct
    let from_impl =
        generate_from_impl_for_struct(&input.name, input.source_type.as_ref(), &input.fields)?;
    all_from_impls.push(from_impl);

    // Generate From implementations for nested structs
    generate_nested_from_implementations(input, all_from_impls)?;
    Ok(())
}

/// Generate From implementation for a specific struct
fn generate_from_impl_for_struct(
    struct_name: &Ident,
    source_type: Option<&Type>,
    fields: &[FieldDefinition],
) -> syn::Result<TokenStream2> {
    let field_mappings = fields.iter().map(|field| {
        let field_name = &field.name;
        let mapping = generate_field_mapping(field);
        quote! {
            #field_name: #mapping,
        }
    });

    // Require explicit source type - no more hardcoded inference
    let source_type_ident = source_type.ok_or_else(|| {
        syn::Error::new(
            struct_name.span(),
            format!("Source type must be explicitly specified for struct '{}'. Use: select_struct!({} from YourSourceType {{ ... }})",
                    struct_name, struct_name)
        )
    })?.clone();

    Ok(quote! {
        impl From<#source_type_ident> for #struct_name {
            fn from(selected: #source_type_ident) -> Self {
                Self {
                    #(#field_mappings)*
                }
            }
        }
    })
}

/// Generate field mapping for a specific field
fn generate_field_mapping(field: &FieldDefinition) -> TokenStream2 {
    let field_name = &field.name;

    match &field.field_type {
        FieldType::Primitive(_ty) => {
            // Direct type conversion - let Rust's type system handle it
            quote! {
                selected.#field_name.unwrap()
            }
        }
        FieldType::Vec(inner) => {
            let inner_mapping = generate_field_mapping_for_type(inner);
            quote! {
                selected.#field_name
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| #inner_mapping)
                    .collect()
            }
        }
        FieldType::Option(inner) => {
            let inner_mapping = generate_field_mapping_for_type(inner);
            quote! {
                selected.#field_name.map(|item| #inner_mapping)
            }
        }
        FieldType::Nested(nested) => {
            let nested_name = &nested.name;
            // Special handling for count fields
            if nested_name.to_string().contains("Count") {
                quote! {
                    #nested_name::from(selected._count.unwrap())
                }
            } else {
                quote! {
                    #nested_name::from(selected.#field_name.unwrap())
                }
            }
        }
    }
}

/// Generate field mapping for a field type
fn generate_field_mapping_for_type(field_type: &FieldType) -> TokenStream2 {
    match field_type {
        FieldType::Primitive(_) => {
            quote! { item }
        }
        FieldType::Vec(inner) => {
            let inner_mapping = generate_field_mapping_for_type(inner);
            quote! {
                item.into_iter().map(|sub_item| #inner_mapping).collect()
            }
        }
        FieldType::Option(inner) => {
            let inner_mapping = generate_field_mapping_for_type(inner);
            quote! {
                item.map(|sub_item| #inner_mapping)
            }
        }
        FieldType::Nested(nested) => {
            let nested_name = &nested.name;
            quote! {
                #nested_name::from(item)
            }
        }
    }
}

/// Generate From implementations for nested structs
fn generate_nested_from_implementations(
    input: &SelectStructInput,
    all_from_impls: &mut Vec<TokenStream2>,
) -> syn::Result<()> {
    for field in &input.fields {
        match &field.field_type {
            FieldType::Nested(nested) => {
                let from_impl = generate_from_impl_for_struct(
                    &nested.name,
                    nested.source_type.as_ref(),
                    &nested.fields,
                )?;
                all_from_impls.push(from_impl);
                generate_nested_from_implementations(nested, all_from_impls)?;
            }
            FieldType::Vec(inner) => {
                generate_from_impl_for_field_type(inner, all_from_impls)?;
            }
            FieldType::Option(inner) => {
                generate_from_impl_for_field_type(inner, all_from_impls)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Generate From implementation for a field type
fn generate_from_impl_for_field_type(
    field_type: &FieldType,
    all_from_impls: &mut Vec<TokenStream2>,
) -> syn::Result<()> {
    match field_type {
        FieldType::Nested(nested) => {
            let from_impl = generate_from_impl_for_struct(
                &nested.name,
                nested.source_type.as_ref(),
                &nested.fields,
            )?;
            all_from_impls.push(from_impl);
            generate_nested_from_implementations(nested, all_from_impls)?;
        }
        FieldType::Vec(inner) => {
            generate_from_impl_for_field_type(inner, all_from_impls)?;
        }
        FieldType::Option(inner) => {
            generate_from_impl_for_field_type(inner, all_from_impls)?;
        }
        _ => {}
    }
    Ok(())
}
