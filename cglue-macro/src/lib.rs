extern crate proc_macro;

mod gen;
mod util;

use gen::trait_groups::TraitGroup;
use proc_macro::TokenStream;
use quote::ToTokens;
use quote::{format_ident, quote};
use syn::*;

#[proc_macro]
pub fn cglue_trait_group(args: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TraitGroup);
    args.create_group().into()
}

#[proc_macro]
pub fn cglue_obj(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as ExprCast);

    let ident = cast.expr;
    let target = cast.ty;

    let target = format_ident!("CGlueTraitObj{}", target.to_token_stream().to_string());

    let gen = quote! {
        #target::from(#ident).into_opaque()
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
