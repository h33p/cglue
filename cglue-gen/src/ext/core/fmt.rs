use proc_macro2::TokenStream;
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(_parent_path: &Path, _out: &mut Vec<(Path, TokenStream)>) {
    //let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));

    /*out.push((
        cur_path,
        quote! {
            // Debug is not FFI-safe at the moment, due to Formatter
            pub trait Debug {
                #[int_result]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error>;
            }
        },
    ));*/
}

pub fn get_exports(_parent_path: &Path, _exports: &mut HashMap<Ident, Path>) {
    //let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));
    //exports.insert(format_ident!("Debug"), cur_path);
}
