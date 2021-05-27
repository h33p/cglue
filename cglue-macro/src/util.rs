use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::parse::ParseStream;
use syn::*;

pub fn crate_path() -> TokenStream {
    let found_crate = crate_name("cglue").expect("cglue found in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => {
            let has_doc_env = std::env::vars()
                .find(|(k, _)| {
                    k == "UNSTABLE_RUSTDOC_TEST_LINE" || k == "UNSTABLE_RUSTDOC_TEST_PATH"
                })
                .is_some();

            if has_doc_env {
                quote!(::cglue)
            } else {
                quote!(crate)
            }
        }
        FoundCrate::Name(name) => quote!(::#name),
    }
}

/// Parse an input stream that is either a single Ident, or a list of Idents surrounded by braces.
pub fn parse_maybe_braced_idents(input: ParseStream) -> Result<Vec<Ident>> {
    let mut ret = vec![];

    if let Ok(braces) = syn::group::parse_braces(&input) {
        let content = braces.content;

        while !content.is_empty() {
            let ident = content.parse()?;

            ret.push(ident);

            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
    } else {
        ret.push(input.parse()?)
    }

    Ok(ret)
}
