#![crate_type = "proc-macro"]
#![allow(dead_code)]
#![allow(unused_variables)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data};

use std::collections::HashSet;
use std::sync::Mutex;

mod common;
mod entity;
mod relation;

lazy_static::lazy_static! {
    static ref ENTITIES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let name_str = name.to_string();
    ENTITIES.lock().unwrap().insert(name_str.clone());

    match &ast.data {
        Data::Struct(_) => TokenStream::from(entity::generate_entity(ast)),
        Data::Enum(_) => TokenStream::from(relation::generate_relation(ast)),
        _ => panic!("Caustics can only be derived on structs and enums"),
    }
}
