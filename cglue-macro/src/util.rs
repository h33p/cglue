use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

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
