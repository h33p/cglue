use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));

    out.push((
        cur_path,
        quote! {
            pub trait Debug {
                #[int_result]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error>;
            }
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));
    exports.insert(format_ident!("Debug"), cur_path);
}
