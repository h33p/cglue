use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("convert"));

    out.push((
        cur_path,
        quote! {
            pub trait AsRef<T> {
                fn as_ref(&self) -> &T;
            }
            pub trait AsMut<T> {
                fn as_mut(&mut self) -> &mut T;
            }
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("convert"));
    exports.insert(format_ident!("AsRef"), cur_path.clone());
    exports.insert(format_ident!("AsMut"), cur_path);
}
