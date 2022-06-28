use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::__private::parse_braces;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Colon2;
use syn::token::Comma;
use syn::*;

pub fn void_type() -> TokenStream {
    let crate_path = crate_path();
    quote!(#crate_path::trait_group::c_void)
}

pub fn crate_path() -> TokenStream {
    let (col, ident) = crate_path_ident();
    quote!(#col #ident)
}

pub fn crate_path_ident() -> (Option<Colon2>, Ident) {
    match crate_path_fixed() {
        Some(FoundCrate::Itself) => (None, format_ident!("crate")),
        Some(FoundCrate::Name(name)) => (Some(Default::default()), format_ident!("{}", name)),
        None => (None, format_ident!("cglue")),
    }
}

pub fn crate_path_fixed() -> Option<FoundCrate> {
    let found_crate = crate_name("cglue").ok()?;

    let ret = match found_crate {
        FoundCrate::Itself => {
            let has_doc_env = std::env::vars().any(|(k, _)| {
                k == "UNSTABLE_RUSTDOC_TEST_LINE" || k == "UNSTABLE_RUSTDOC_TEST_PATH"
            });

            if has_doc_env {
                FoundCrate::Name("cglue".to_string())
            } else {
                FoundCrate::Itself
            }
        }
        x => x,
    };

    Some(ret)
}

/// Parse an input stream that is either a single Ident, or a list of Idents surrounded by braces.
pub fn parse_maybe_braced<T: Parse>(input: ParseStream) -> Result<Vec<T>> {
    let mut ret = vec![];

    if let Ok(braces) = parse_braces(input) {
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

pub fn split_path_ident(in_path: &Path) -> Result<(Path, Ident, GenericsOut)> {
    let mut path = Path {
        leading_colon: in_path.leading_colon,
        segments: Default::default(),
    };

    let mut ident = None;

    let mut generics = None;

    for part in in_path.segments.pairs() {
        match part {
            punctuated::Pair::Punctuated(p, _) => {
                path.segments.push_value(p.clone());
                path.segments.push_punct(Default::default());
            }
            punctuated::Pair::End(p) => {
                if let PathArguments::AngleBracketed(arg) = &p.arguments {
                    generics = Some(arg.args.clone());
                }
                ident = Some(p.ident.clone());
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

            last.map(|l| {
                let s = &l.ident.to_string();
                ["NonNull", "Box"].contains(&s.as_str())
                    || custom_types.contains(&s.as_str())
                    || (s.starts_with("NonZero")
                        && [
                            "I8", "U8", "I16", "U16", "I32", "U32", "I64", "U64", "I128", "U128",
                        ]
                        .contains(&s.split_at("NonZero".len()).1))
            }) == Some(true)
        }
        _ => false,
    }
}
