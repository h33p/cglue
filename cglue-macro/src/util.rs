use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::parse::ParseStream;
use syn::*;

pub fn crate_path() -> TokenStream {
    let found_crate = crate_name("cglue").expect("cglue found in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => {
            let has_doc_env = std::env::vars().any(|(k, _)| {
                k == "UNSTABLE_RUSTDOC_TEST_LINE" || k == "UNSTABLE_RUSTDOC_TEST_PATH"
            });

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

/// Checks whether the type could be null pointer optimizable.
///
/// Note that this is not a foolproof solution, and might have both false positives and negatives.
///
/// # Arguments
///
/// * `ty` - input type to check against.
/// * `custom_types` - custom types that are NPO-able.
pub fn is_null_pointer_optimizable(ty: &Type, custom_types: &[&'static str]) -> bool {
    match ty {
        Type::Reference(_) => true,
        Type::BareFn(_) => true,
        Type::Path(path) => {
            let last = path.path.segments.last();

            matches!(
                last.map(|l| {
                    let s = &l.ident.to_string();
                    ["NonNull", "Box"].contains(&s.as_str())
                        || custom_types.contains(&s.as_str())
                        || (s.starts_with("NonZero")
                            && [
                                "I8", "U8", "I16", "U16", "I32", "U32", "I64", "U64", "I128",
                                "U128",
                            ]
                            .contains(&s.split_at("NonZero".len()).1))
                }),
                Some(true)
            )
        }
        _ => false,
    }
}
