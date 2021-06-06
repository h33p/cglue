pub mod core;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use syn::{
    parse::{ParseStream, Parser},
    *,
};

pub fn get_exports() -> HashMap<Ident, Path> {
    let mut exports = HashMap::new();

    let mut ext_path: Path = parse2(quote!(::ext)).unwrap();
    ext_path.segments.push_punct(Default::default());

    core::get_exports(&ext_path, &mut exports);

    exports
}

pub fn get_store() -> HashMap<(Path, Ident), ItemTrait> {
    let mut token_list = vec![];

    let mut ext_path: Path = parse2(quote!(::ext)).unwrap();
    ext_path.segments.push_punct(Default::default());

    core::get_impl(&ext_path, &mut token_list);

    let mut parsed_traits = HashMap::new();

    for (path, body) in token_list {
        let traits = Parser::parse2(parse_traits, body).expect("Failed to parse traits");

        for tr in traits {
            parsed_traits.insert((path.clone(), tr.ident.clone()), tr);
        }
    }

    parsed_traits
}

fn subpath_to_tokens(path: &Path, skip: usize) -> TokenStream {
    let mut out = TokenStream::new();

    for seg in path.segments.pairs().skip(skip) {
        match seg {
            punctuated::Pair::Punctuated(p, punc) => {
                out.extend(quote!(#p #punc));
            }
            punctuated::Pair::End(p) => {
                out.extend(quote!(#p));
            }
        }
    }

    out
}

type Modules = HashMap<usize, HashMap<Path, (TokenStream, Vec<Ident>)>>;

fn impl_mod(
    path: &Path,
    name: &Ident,
    depth: usize,
    mut mod_impl: TokenStream,
    children: Vec<Ident>,
    modules: &mut Modules,
) -> TokenStream {
    let child_depth = depth + 1;

    for ident in children {
        let mut path = path.clone();

        let name = ident.clone();

        path.segments.push_value(PathSegment {
            ident,
            arguments: Default::default(),
        });

        path.segments.push_punct(Default::default());

        let (ts, children) = modules
            .get_mut(&child_depth)
            .expect("Module depth not found")
            .remove(&path)
            .expect("Child module not found");

        mod_impl.extend(impl_mod(&path, &name, child_depth, ts, children, modules));
    }

    quote! {
        pub mod #name {
            #mod_impl
        }
    }
}

/// Remaps all Ident paths that are in the export list to become ::ext::Ident
pub fn prelude_remap(path: Path) -> Path {
    if let Some(ident) = path.get_ident().cloned() {
        if let Some(path) = get_exports().get(&ident) {
            let mut new_path = path.clone();

            new_path.segments.push(PathSegment {
                ident,
                arguments: Default::default(),
            });

            new_path
        } else {
            path
        }
    } else {
        path
    }
}

/// Returns the absolute export path if ident is in exports, and path is empty.
pub fn prelude_remap_with_ident(path: Path, ident: &Ident) -> Path {
    if !path.segments.is_empty() {
        path
    } else if let Some(path) = get_exports().get(&ident) {
        path.clone()
    } else {
        path
    }
}

/// Remaps all ::ext:: paths to become ::cglue::ext:: paths.
pub fn ext_abs_remap(path: Path) -> Path {
    let mut iter = path.segments.iter();
    if let (Some(_), Some(seg)) = (path.leading_colon, iter.next()) {
        if seg.ident == "ext" {
            let (leading_colon, ident) = crate::util::crate_path_ident();

            let mut new_path = Path {
                leading_colon,
                segments: Default::default(),
            };

            new_path.segments.push_value(PathSegment {
                ident,
                arguments: Default::default(),
            });

            new_path.segments.push_punct(Default::default());

            std::mem::drop(iter);

            new_path.segments.extend(path.segments.into_pairs());

            /*std::mem::drop(iter);

            let (seg, punct) = path.segments.pop().unwrap().into_tuple();

            new_path.segments.push_value(seg);

            if let Some(punct) = punct {
                new_path.segments.push_punct(punct);
            }*/

            new_path
        } else {
            std::mem::drop(iter);
            path
        }
    } else {
        std::mem::drop(iter);
        path
    }
}

/// Implement the external trait store.
pub fn impl_store() -> TokenStream {
    let mut out = TokenStream::new();

    let exports = get_exports();
    let store = get_store();

    let mut modules = HashMap::<usize, HashMap<Path, (TokenStream, Vec<Ident>)>>::new();

    // Re-export everything
    for (k, v) in exports.into_iter() {
        let subpath = subpath_to_tokens(&v, 1);

        for ident in [
            "",
            "Ext",
            "CGlueVtbl",
            "CGlueRetTmp",
            "OpaqueCGlueVtbl",
            "CGlueBase",
            "CGlueBox",
            "CGlueMut",
            "CGlueRef",
        ]
        .iter()
        .map(|p| format_ident!("{}{}", p, k))
        {
            out.extend(quote!(pub use self:: #subpath #ident;));
        }
    }

    for ((p, _), t) in store.into_iter() {
        // exclude :: ext :: segment, and the whole layer altogether
        let segments = p.segments.len();

        assert!(segments > 1, "External traits defined in external root!");

        let depth = segments - 2;

        push_to_parent(depth, &p, &mut modules);

        let (module, _) = modules
            .entry(depth)
            .or_default()
            .entry(p.clone())
            .or_default();

        let name = &t.ident;

        module.extend(quote! {
            pub use #name;

            #[::cglue_macro::cglue_trait_ext]
            #t
        });
    }

    if let Some(root) = modules.remove(&0) {
        for (p, (ts, children)) in root {
            let name = &p.segments.iter().next_back().unwrap().ident;
            out.extend(impl_mod(&p, &name, 0, ts, children, &mut modules))
        }
    } else if !modules.is_empty() {
        panic!("Module implementations defined, but everything is disjoint from the root!");
    }

    out
}

fn push_to_parent(depth: usize, path: &Path, modules: &mut Modules) {
    if depth == 0 {
        return;
    }

    let child_depth = depth - 1;

    let mut parent_path = path.clone();
    let my_ident = parent_path
        .segments
        .pop()
        .map(punctuated::Pair::into_value)
        .unwrap()
        .ident;

    let entry = modules
        .entry(child_depth)
        .or_default()
        .entry(parent_path.clone());

    match entry {
        Entry::Occupied(mut e) => e.get_mut().1.push(my_ident),
        Entry::Vacant(_) => {
            push_to_parent(child_depth, &parent_path, modules);
            let (_, children) = modules
                .entry(child_depth)
                .or_default()
                .entry(parent_path)
                .or_default();
            children.push(my_ident);
        }
    }
}

fn parse_traits(input: ParseStream) -> Result<Vec<ItemTrait>> {
    let mut out = vec![];

    while !input.is_empty() {
        let val = input.parse()?;

        out.push(val);
    }

    Ok(out)
}

fn join_paths(path: &Path, ident: Ident) -> Path {
    let mut ret = path.clone();

    if !ret.segments.empty_or_trailing() {
        ret.segments.push_punct(Default::default());
    }

    ret.segments.push_value(PathSegment {
        ident,
        arguments: Default::default(),
    });

    ret.segments.push_punct(Default::default());

    ret
}
