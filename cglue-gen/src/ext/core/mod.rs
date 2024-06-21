pub mod clone;
pub mod convert;
pub mod fmt;
pub mod future;

use proc_macro2::TokenStream;
use quote::format_ident;
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::join_paths(parent_path, format_ident!("core"));
    clone::get_impl(&cur_path, out);
    fmt::get_impl(&cur_path, out);
    convert::get_impl(&cur_path, out);
    #[cfg(feature = "task")]
    future::get_impl(&cur_path, out);
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::join_paths(parent_path, format_ident!("core"));
    clone::get_exports(&cur_path, exports);
    fmt::get_exports(&cur_path, exports);
    convert::get_exports(&cur_path, exports);
    #[cfg(feature = "task")]
    future::get_exports(&cur_path, exports);
}
