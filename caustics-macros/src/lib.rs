#![crate_type = "proc-macro"]

extern crate proc_macro;
mod caustics;

use proc_macro::TokenStream;

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(input: TokenStream) -> TokenStream {
    let input: proc_macro2::TokenStream = input.into();
    caustics::generate_caustics_impl(input).into()
}