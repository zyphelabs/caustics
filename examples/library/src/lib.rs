#![feature(decl_macro)]
pub mod entities;

// Include the generated client directly in the root module
include!(concat!(env!("OUT_DIR"), "/caustics_client_library.rs"));
