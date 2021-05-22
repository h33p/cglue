extern crate proc_macro;

mod gen;

use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro]
pub fn cglue_trait_group(args: TokenStream) -> TokenStream {
    let gen = quote! {};

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
