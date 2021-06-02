use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
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
pub fn parse_maybe_braced<T: Parse>(input: ParseStream) -> Result<Vec<T>> {
    let mut ret = vec![];

    if let Ok(braces) = syn::group::parse_braces(&input) {
        let content = braces.content;

        while !content.is_empty() {
            let val = content.parse()?;

            ret.push(val);

            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
    } else {
        ret.push(input.parse()?)
    }

    Ok(ret)
}

pub type GenericsOut = Option<Punctuated<GenericArgument, Comma>>;

pub fn split_path_ident(in_path: Path) -> Result<(TokenStream, Ident, GenericsOut)> {
    let mut path = in_path.leading_colon.to_token_stream();

    let mut ident = None;

    let mut generics = None;

    for part in in_path.segments.into_pairs() {
        match part {
            punctuated::Pair::Punctuated(p, punc) => path.extend(quote!(#p #punc)),
            punctuated::Pair::End(p) => {
                if let PathArguments::AngleBracketed(arg) = p.arguments {
                    generics = Some(arg.args);
                }
                ident = Some(p.ident);
            }
        }
    }

    let ident =
        ident.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "Ident not found!"))?;

    Ok((path, ident, generics))
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
