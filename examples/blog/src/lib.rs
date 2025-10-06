#![cfg_attr(feature = "select", feature(decl_macro))]

pub mod entities;

include!(concat!(env!("OUT_DIR"), "/caustics_client_blog.rs"));
