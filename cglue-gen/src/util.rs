use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, HashSet};
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

pub fn parse_punctuated<T: Parse, P: Parse>(input: ParseStream) -> Punctuated<T, P> {
    let mut punct = Punctuated::new();

    while let Ok(arg) = input.parse::<T>() {
        punct.push_value(arg);

        if let Ok(comma) = input.parse::<P>() {
            punct.push_punct(comma);
        } else {
            break;
        }
    }

    punct
}

pub fn parse_brace_content(input: ParseStream) -> Result<syn::parse::ParseBuffer> {
    let content;
    syn::braced!(content in input);
    Ok(content)
}

/// Parse an input stream that is either a single Ident, or a list of Idents surrounded by braces.
pub fn parse_maybe_braced<T: Parse>(input: ParseStream) -> Result<Vec<T>> {
    let mut ret = vec![];

    if let Ok(content) = parse_brace_content(input) {
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

/// Extract heuristically generic arguments from the type.
///
/// This function looks for AngleBracketed path arguments and saves the last one.
pub fn extract_generics(ty: &mut Type) -> GenericsOut {
    recurse_type_to_path(ty, |path| {
        let mut generics = None;
        for part in path.segments.pairs() {
            if let punctuated::Pair::End(p) = part {
                if let PathArguments::AngleBracketed(arg) = &p.arguments {
                    generics = Some(arg.args.clone());
                }
            }
        }
        generics
    })
}

/// Recurse down to TypePath and call closure.
pub fn recurse_type_to_path<T>(
    ty: &mut Type,
    func: impl FnOnce(&mut Path) -> Option<T>,
) -> Option<T> {
    match ty {
        Type::Path(TypePath { path, .. }) => func(path),
        Type::Array(TypeArray { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        Type::Group(TypeGroup { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        Type::Paren(TypeParen { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        Type::Ptr(TypePtr { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        Type::Reference(TypeReference { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        Type::Slice(TypeSlice { elem, .. }) => recurse_type_to_path(&mut *elem, func),
        _ => None,
        //Type::Tuple(TypeTuple),
        //Type::Verbatim(TokenStream),
    }
}

pub fn map_lifetimes<T>(
    lifetimes: &mut Punctuated<Lifetime, T>,
    map: &BTreeMap<Lifetime, Lifetime>,
) {
    for lt in lifetimes.iter_mut() {
        if let Some(target) = map.get(lt) {
            *lt = target.clone();
        }
    }
}

pub fn map_lifetime_defs<T>(
    lifetimes: &mut Punctuated<LifetimeDef, T>,
    map: &BTreeMap<Lifetime, Lifetime>,
) {
    for lt in lifetimes.iter_mut() {
        if let Some(target) = map.get(&lt.lifetime) {
            lt.lifetime = target.clone();
        }
        map_lifetimes(&mut lt.bounds, map);
    }
}

pub fn remap_lifetime_defs<T: Clone>(
    lifetimes: &Punctuated<LifetimeDef, T>,
    map: &BTreeMap<Lifetime, Lifetime>,
) -> Punctuated<LifetimeDef, T> {
    let mut lifetimes = lifetimes.clone();
    map_lifetime_defs(&mut lifetimes, map);
    lifetimes
}

/// Recursively remap lifetimes of a type.
pub fn remap_type_lifetimes(ty: &mut Type, map: &BTreeMap<Lifetime, Lifetime>) {
    match ty {
        Type::Reference(TypeReference {
            elem,
            ref mut lifetime,
            ..
        }) => {
            if let Some(new_lt) = lifetime.as_ref().and_then(|lt| map.get(lt)) {
                *lifetime = Some(new_lt.clone());
            }
            remap_type_lifetimes(&mut *elem, map)
        }
        Type::Path(TypePath { path, qself, .. }) => {
            if let Some(s) = qself.as_mut() {
                remap_type_lifetimes(&mut s.ty, map);
            }
            for seg in path.segments.iter_mut() {
                match &mut seg.arguments {
                    PathArguments::AngleBracketed(args) => {
                        for arg in args.args.iter_mut() {
                            match arg {
                                GenericArgument::Lifetime(lt) => {
                                    if let Some(new_lt) = map.get(lt) {
                                        *lt = new_lt.clone();
                                    }
                                }
                                GenericArgument::Type(ty) => remap_type_lifetimes(ty, map),
                                _ => (),
                            }
                        }
                    }
                    PathArguments::Parenthesized(args) => {
                        for arg in args.inputs.iter_mut() {
                            remap_type_lifetimes(arg, map);
                        }
                        if let ReturnType::Type(_, ty) = &mut args.output {
                            remap_type_lifetimes(&mut *ty, map);
                        }
                    }
                    _ => (),
                }
            }
        }
        Type::Array(TypeArray { elem, .. }) => remap_type_lifetimes(&mut *elem, map),
        Type::Group(TypeGroup { elem, .. }) => remap_type_lifetimes(&mut *elem, map),
        Type::Paren(TypeParen { elem, .. }) => remap_type_lifetimes(&mut *elem, map),
        Type::Ptr(TypePtr { elem, .. }) => remap_type_lifetimes(&mut *elem, map),
        Type::Slice(TypeSlice { elem, .. }) => remap_type_lifetimes(&mut *elem, map),
        Type::Tuple(TypeTuple { elems, .. }) => {
            for elem in elems.iter_mut() {
                remap_type_lifetimes(elem, map)
            }
        }
        _ => (),
        //Type::Tuple(TypeTuple),
        //Type::Verbatim(TokenStream),
    }
}

pub fn merge_lifetime_declarations(
    a: &Punctuated<LifetimeDef, Comma>,
    b: &Punctuated<LifetimeDef, Comma>,
) -> Punctuated<LifetimeDef, Comma> {
    let mut life_declare = Punctuated::new();
    let mut life_declared = HashSet::<&Ident>::new();

    for val in &[a, b] {
        for life in val.pairs() {
            let (val, punct) = life.into_tuple();
            if life_declared.contains(&val.lifetime.ident) {
                continue;
            }
            life_declare.push_value(val.clone());
            if let Some(punct) = punct {
                life_declare.push_punct(*punct);
            }
            life_declared.insert(&val.lifetime.ident);
        }
    }

    life_declare
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
