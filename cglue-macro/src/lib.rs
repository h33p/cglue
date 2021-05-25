extern crate proc_macro;

mod gen;
mod util;

use gen::single_obj::ObjStruct;
use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro]
pub fn cglue_trait_group(_args: TokenStream) -> TokenStream {
    let gen = quote! {};
    gen.into()
}

#[proc_macro]
pub fn cglue_obj(args: TokenStream) -> TokenStream {
    let ObjStruct { ident, target } = parse_macro_input!(args as ObjStruct);

    let target: proc_macro2::TokenStream = format!("CGlueTraitObj{}", target.to_string())
        .parse()
        .unwrap();

    let gen = quote! {
        #target::from(&mut #ident).into_opaque()
    };

    gen.into()
}

#[proc_macro_attribute]
pub fn cglue_trait(_args: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);

    let trait_def = gen::traits::gen_trait(&tr);

    let gen = quote! {
        #tr
        #trait_def
    };

    gen.into()
}
