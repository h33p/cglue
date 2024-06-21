use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::{AssocType, CustomFuncImpl, ParsedFunc, WrappedType};
use super::generics::{GenericType, ParsedGenerics};

use quote::*;
use syn::{
    parse::*,
    punctuated::Punctuated,
    token::{Add, Comma},
    *,
};

pub struct PathTokens {
    lifetime: Option<Lifetime>,
    path: Path,
    tokens: TokenStream,
}

impl Parse for PathTokens {
    fn parse(input: ParseStream) -> Result<Self> {
        let lifetime = if let Ok(lifetime) = input.parse() {
            input.parse::<Comma>()?;
            Some(lifetime)
        } else {
            None
        };

        let path = input.parse()?;
        let tokens = input.parse()?;

        Ok(Self {
            lifetime,
            path,
            tokens,
        })
    }
}

// TODO: Add dynamic setting of Send / Sync
pub fn ctx_bound() -> TokenStream {
    let crate_path = crate::util::crate_path();
    quote!(#crate_path::trait_group::ContextBounds)
}

pub fn cglue_c_opaque_bound() -> TokenStream {
    let crate_path = crate::util::crate_path();
    quote!(CGlueC::OpaqueTarget: #crate_path::trait_group::Opaquable + #crate_path::trait_group::CGlueObjBase,)
}

pub fn process_item(
    (ty_def, ty_bounds, ty_where_clause, ty_attrs): (
        &Option<AssocType>,
        &Punctuated<TypeParamBound, Add>,
        Option<&WhereClause>,
        &[Attribute],
    ),
    trait_name: &Ident,
    generics: &ParsedGenerics,
    trait_type_defs: &mut TokenStream,
    types: &mut BTreeMap<Option<AssocType>, WrappedType>,
    assoc_types: &mut BTreeMap<AssocType, Punctuated<TypeParamBound, Add>>,
    crate_path: &TokenStream,
) {
    let c_void = crate::util::void_type();

    let static_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("static"),
    };

    let cglue_a_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("cglue_a"),
    };

    let cglue_b_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("cglue_b"),
    };

    let cglue_c_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("cglue_c"),
    };

    let mut lifetime_bounds = ty_bounds.iter().filter_map(|b| match b {
        TypeParamBound::Lifetime(lt) => Some(lt),
        _ => None,
    });

    let orig_lifetime_bound = lifetime_bounds.next();

    // When anonymous lifetime is passed, use self lifetime
    let orig_lifetime_bound = if orig_lifetime_bound.map(|lt| lt.ident == "_") == Some(true) {
        Some(&cglue_c_lifetime)
    } else {
        orig_lifetime_bound
    };

    if lifetime_bounds.next().is_some() {
        panic!("Traits with multiple lifetime bounds are not supported!");
    }

    let mut wrapped = false;

    for attr in ty_attrs {
        let s = attr.path.to_token_stream().to_string();

        let x = s.as_str();

        match x {
            "wrap_with" => {
                wrapped = true;
                let new_ty = attr
                    .parse_args::<GenericType>()
                    .expect("Invalid type in wrap_with.");

                if let Some(ty_def) = ty_def {
                    trait_type_defs.extend(quote!(type #ty_def = #new_ty #ty_where_clause;));
                }

                types.insert(
                    ty_def.clone(),
                    WrappedType {
                        ty: new_ty.clone(),
                        ty_ret_tmp: Some(new_ty),
                        ty_static: None,
                        return_conv: None,
                        lifetime_bound: None,
                        lifetime_type_bound: None,
                        other_bounds: None,
                        other_bounds_simple: None,
                        impl_return_conv: None,
                        inject_ret_tmp: false,
                        unbounded_hrtb: false,
                    },
                );
            }
            "return_wrap" => {
                wrapped = true;
                let closure = attr
                    .parse_args::<ExprClosure>()
                    .expect("A valid closure must be supplied accepting the wrapped type!");

                types
                    .get_mut(ty_def)
                    .expect("Type must be first wrapped with #[wrap_with(T)] atribute.")
                    .return_conv = Some(closure);
            }
            "wrap_with_obj"
            | "wrap_with_obj_ref"
            | "wrap_with_obj_mut"
            | "wrap_with_group"
            | "wrap_with_group_ref"
            | "wrap_with_group_mut" => {
                wrapped = true;
                let mut new_ty = attr
                    .parse_args::<GenericType>()
                    .expect("Invalid type in wrap_with.");

                let target_ty = new_ty.clone();
                let target = new_ty.target.clone();

                //if target.to_string() == "G"

                if ["wrap_with_obj", "wrap_with_obj_ref", "wrap_with_obj_mut"].contains(&x) {
                    new_ty.target = format_ident!("{}Base", target.to_string()).to_token_stream();
                }

                // These variables model a `CGlueF::#ty_def: Into<SomeGroup>` bound.
                let mut from_new_ty = new_ty.clone();
                let mut from_new_ty_ref = TokenStream::new();
                let mut from_new_ty_simple = new_ty.clone();
                let mut from_new_ty_simple_ref = TokenStream::new();

                let mut new_ty_static = new_ty.clone();

                // Inject static bound when we wrap owned objects, because we can not ensure their
                // safety like that.
                let lifetime_bound = if orig_lifetime_bound.is_none()
                    && (x == "wrap_with_group" || x == "wrap_with_obj")
                {
                    Some(&static_lifetime)
                } else {
                    orig_lifetime_bound
                };

                let lifetime = lifetime_bound.unwrap_or(&static_lifetime);

                // Insert the object lifetime at the start
                new_ty.push_lifetime_start(lifetime);
                new_ty_static.push_lifetime_start(&static_lifetime);

                let from_lifetime = if (x == "wrap_with_group" || x == "wrap_with_obj")
                    && lifetime == &static_lifetime
                {
                    lifetime
                } else {
                    &cglue_b_lifetime
                };

                from_new_ty.push_lifetime_start(from_lifetime);

                let from_lifetime_simple = if (x == "wrap_with_group" || x == "wrap_with_obj")
                    && lifetime == &static_lifetime
                {
                    lifetime
                } else {
                    &cglue_a_lifetime
                };

                from_new_ty_simple.push_lifetime_start(from_lifetime_simple);

                let gen_use = &generics.gen_use;

                let mut new_ty_trait_impl = new_ty.clone();
                let mut new_ty_ret_tmp = new_ty.clone();

                let (other_bounds, other_bounds_simple) = {
                    let (type_bounds, type_bounds_simple) = {
                        // Here we must inject a lifetime, if the trait has no lifetime,
                        // and its a group we are wrapping
                        let hrtb_lifetime = quote!(#cglue_b_lifetime);

                        let hrtb_lifetime_use = if generics.life_use.is_empty() {
                            quote!()
                        } else {
                            quote!(#from_lifetime)
                        };

                        let simple_lifetime_use = if generics.life_use.is_empty() {
                            quote!()
                        } else {
                            quote!(#from_lifetime_simple)
                        };

                        let cglue_f_tys = ty_def.as_ref().map(|ty_def| {
                            let ty_def = ty_def.remap_for_hrtb();
                            (
                                quote!(<CGlueC::ObjType as #trait_name<#hrtb_lifetime_use #gen_use>>::#ty_def),
                                quote!(<CGlueC::ObjType as #trait_name<#simple_lifetime_use #gen_use>>::#ty_def),
                            )
                        });

                        let mut new_ty_hrtb = from_new_ty.clone();
                        let mut new_ty_simple = from_new_ty_simple.clone();

                        if x == "wrap_with_group" || x == "wrap_with_obj" {
                            // <CGlueO::ContType as crate::trait_group::CGlueObjBase>::Context
                            new_ty.push_types_start(
                                quote!(#crate_path::boxed::CBox<#lifetime, #c_void>, CGlueC::Context, ),
                            );
                            new_ty_ret_tmp.push_types_start(
                                quote!(#crate_path::boxed::CBox<#lifetime, #c_void>, CGlueCtx, ),
                            );
                            new_ty_trait_impl.push_types_start(
                                quote!(#crate_path::boxed::CBox<#lifetime, #c_void>, <CGlueO::ContType as #crate_path::trait_group::CGlueObjBase>::Context, ),
                            );
                            new_ty_hrtb.push_types_start(
                                quote!(#crate_path::boxed::CBox<#from_lifetime, #c_void>, CGlueC::Context, ),
                            );
                            new_ty_simple.push_types_start(
                                quote!(#crate_path::boxed::CBox<#from_lifetime_simple, #c_void>, CGlueC::Context, ),
                            );
                            new_ty_static.push_types_start(
                                quote!(#crate_path::boxed::CBox<'static, #c_void>, CGlueCtx, ),
                            );

                            if let Some((cglue_f_ty_def, cglue_f_ty_simple_ident)) = &cglue_f_tys {
                                from_new_ty.push_types_start(
                                    quote!(#crate_path::boxed::CBox<#from_lifetime, #cglue_f_ty_def>, CGlueC::Context, ),
                                );
                                from_new_ty_simple.push_types_start(
                                    quote!(#crate_path::boxed::CBox<#from_lifetime_simple, #cglue_f_ty_simple_ident>, CGlueC::Context, ),
                                );
                            }
                        } else if x == "wrap_with_group_ref" || x == "wrap_with_obj_ref" {
                            let no_context = quote!(CGlueC::Context);
                            new_ty.push_types_start(quote!(&#lifetime #c_void, CGlueC::Context, ));
                            new_ty_ret_tmp.push_types_start(quote!(&#lifetime #c_void, CGlueCtx, ));
                            new_ty_trait_impl.push_types_start(
                                quote!(&#lifetime #c_void, <CGlueO::ContType as crate::trait_group::CGlueObjBase>::Context, ),
                            );
                            new_ty_hrtb.push_types_start(
                                quote!(&#from_lifetime #c_void, CGlueC::Context, ),
                            );
                            new_ty_simple.push_types_start(
                                quote!(&#from_lifetime_simple #c_void, CGlueC::Context, ),
                            );
                            new_ty_static.push_types_start(quote!(&'static #c_void, CGlueCtx, ));
                            if let Some((cglue_f_ty_def, cglue_f_ty_simple_ident)) = &cglue_f_tys {
                                from_new_ty.push_types_start(
                                    quote!(&#from_lifetime #cglue_f_ty_def, #no_context, ),
                                );
                                from_new_ty_ref.extend(quote!(&#from_lifetime));
                                from_new_ty_simple.push_types_start(
                                    quote!(&#from_lifetime_simple #cglue_f_ty_simple_ident, #no_context, ),
                                );
                                from_new_ty_simple_ref.extend(quote!(&#from_lifetime_simple));
                            }
                        } else if x == "wrap_with_group_mut" || x == "wrap_with_obj_mut" {
                            let no_context = quote!(CGlueC::Context);
                            new_ty.push_types_start(
                                quote!(&#lifetime mut #c_void, CGlueC::Context, ),
                            );
                            new_ty_ret_tmp
                                .push_types_start(quote!(&#lifetime mut #c_void, CGlueCtx, ));
                            new_ty_trait_impl.push_types_start(
                                quote!(&#lifetime mut #c_void, <CGlueO::ContType as crate::trait_group::CGlueObjBase>::Context, ),
                            );
                            new_ty_hrtb.push_types_start(
                                quote!(&#from_lifetime mut #c_void, CGlueC::Context, ),
                            );
                            new_ty_simple.push_types_start(
                                quote!(&#from_lifetime_simple mut #c_void, CGlueC::Context, ),
                            );
                            new_ty_static
                                .push_types_start(quote!(&'static mut #c_void, CGlueCtx, ));
                            if let Some((cglue_f_ty_def, cglue_f_ty_simple_ident)) = &cglue_f_tys {
                                from_new_ty.push_types_start(
                                    quote!(&#from_lifetime mut #cglue_f_ty_def, #no_context, ),
                                );
                                from_new_ty_ref.extend(quote!(&#from_lifetime mut));
                                from_new_ty_simple.push_types_start(
                                    quote!(&#from_lifetime_simple mut #cglue_f_ty_simple_ident, #no_context, ),
                                );
                                from_new_ty_simple_ref.extend(quote!(&#from_lifetime_simple mut));
                            }
                        } else {
                            unreachable!()
                        }

                        if let Some((cglue_f_ty_def, cglue_f_ty_simple_ident)) = cglue_f_tys {
                            let (ty_ref, ty_ref_simple) = {
                                (
                                    quote!((#from_new_ty_ref #cglue_f_ty_def, CGlueC::Context)),
                                    quote!((#from_new_ty_simple_ref #cglue_f_ty_simple_ident, CGlueC::Context)),
                                )
                            };

                            let type_bounds = quote! {
                                for<#hrtb_lifetime> #ty_ref: Into<#from_new_ty>,
                                for<#hrtb_lifetime> #from_new_ty: #crate_path::trait_group::Opaquable<OpaqueTarget = #new_ty_hrtb>,
                            };

                            let type_bounds_simple = quote! {
                                #ty_ref_simple: Into<#from_new_ty_simple>,
                                #from_new_ty_simple: #crate_path::trait_group::Opaquable<OpaqueTarget = #new_ty_simple>,
                            };
                            (Some(type_bounds), Some(type_bounds_simple))
                        } else if lifetime_bound == Some(&static_lifetime) {
                            (
                                Some(quote!(CGlueC::ObjType: 'static,)),
                                Some(quote!(CGlueC::ObjType: 'static,)),
                            )
                        } else {
                            (None, None)
                        }
                    };

                    if let Some(ty_def) = ty_def {
                        trait_type_defs
                            .extend(quote!(type #ty_def = #new_ty_trait_impl #ty_where_clause;));
                    }

                    (type_bounds, type_bounds_simple)
                };

                let ret_write_unsafe = quote! {
                    // SAFETY:
                    // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                    // returning reference to the object.
                    unsafe {
                        // Transmute sometimes does not work even when same size CGlueTraitObj is used, and
                        // only lifetimes differ. Manually assert sizes, which will result in either a no-op
                        // or forced panic on release builds.
                        assert!(std::mem::size_of_val(ret_tmp) == std::mem::size_of_val(&ret));
                        std::ptr::copy_nonoverlapping(std::mem::transmute(&ret), ret_tmp.as_mut_ptr(), 1);
                        std::mem::forget(ret);
                    }
                };

                let (ret_write, conv_bound) = if lifetime != &static_lifetime {
                    (
                        quote! {
                            unsafe {
                                ret_tmp.as_mut_ptr().write(ret);
                            }
                        },
                        quote!(#lifetime),
                    )
                } else {
                    (ret_write_unsafe.clone(), quote!('cglue_a))
                };

                let target = target_ty;

                let (return_conv, inject_ret_tmp) = match x {
                    "wrap_with_obj" => (
                        parse2(quote!(|ret| trait_obj!((ret, cglue_ctx) as #target)))
                            .expect("Internal closure parsing fail"),
                        false,
                    ),
                    "wrap_with_group" => (
                        parse2(quote!(|ret| group_obj!((ret, cglue_ctx) as #target)))
                            .expect("Internal closure parsing fail"),
                        false,
                    ),
                    "wrap_with_obj_ref" => (
                        parse2(quote!(|ret: &#conv_bound _| {
                            let ret = trait_obj!((ret, cglue_ctx) as #target);
                            #ret_write_unsafe
                            unsafe { &*ret_tmp.as_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_group_ref" => (
                        parse2(quote!(|ret: &#conv_bound _| {
                            let ret = group_obj!((ret, cglue_ctx) as #target);
                            #ret_write_unsafe
                            unsafe { &*ret_tmp.as_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_obj_mut" => (
                        parse2(quote!(|ret: &#conv_bound mut _| {
                            let ret = trait_obj!((ret, cglue_ctx) as #target);
                            #ret_write
                            unsafe { &mut *ret_tmp.as_mut_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_group_mut" => (
                        parse2(quote!(|ret: &#conv_bound mut _| {
                            let ret = group_obj!((ret, cglue_ctx) as #target);
                            #ret_write
                            unsafe { &mut *ret_tmp.as_mut_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    _ => unreachable!(),
                };

                // A very odd hack.
                //
                // On 1.45 compiling sometimes fails when the trait type is already bound for
                // static, but an additional (redundant) static bound is added on top.
                let lifetime_type_bound = if orig_lifetime_bound != Some(&static_lifetime) {
                    lifetime_bound.cloned()
                } else {
                    None
                };

                let lifetime_bound = if lifetime_bound != Some(&static_lifetime) {
                    lifetime_bound.cloned()
                } else {
                    None
                };

                //let lifetime_type_bound = lifetime_bound.clone();

                types.insert(
                    ty_def.clone(),
                    WrappedType {
                        ty: new_ty,
                        ty_ret_tmp: Some(new_ty_ret_tmp),
                        ty_static: Some(new_ty_static),
                        return_conv: Some(return_conv),
                        impl_return_conv: None,
                        lifetime_bound,
                        lifetime_type_bound,
                        other_bounds,
                        other_bounds_simple,
                        inject_ret_tmp,
                        unbounded_hrtb: false,
                    },
                );
            }
            _ => {}
        }
    }

    if let (Some(ty_def), false) = (ty_def, wrapped) {
        let new_ty = parse2(format_ident!("CGlueA{}", ty_def.ident).to_token_stream())
            .expect("Invalid type in unwrapped assoc");

        trait_type_defs.extend(quote!(type #ty_def = #new_ty #ty_where_clause;));

        types.insert(
            Some(ty_def.clone()),
            WrappedType {
                ty: new_ty,
                ty_ret_tmp: None,
                ty_static: None,
                return_conv: None,
                impl_return_conv: None,
                lifetime_bound: None,
                lifetime_type_bound: None,
                other_bounds: None,
                other_bounds_simple: None,
                inject_ret_tmp: false,
                unbounded_hrtb: false,
            },
        );

        assoc_types.insert(ty_def.clone(), ty_bounds.clone());
    }
}

pub fn parse_trait(
    tr: &ItemTrait,
    crate_path: &TokenStream,
    also_parse_vtbl_only: bool,
    mut process_item: impl FnMut(
        (
            &Option<AssocType>,
            &Punctuated<TypeParamBound, Add>,
            Option<&WhereClause>,
            &[Attribute],
        ),
        &Ident,
        &ParsedGenerics,
        &mut TokenStream,
        &mut BTreeMap<Option<AssocType>, WrappedType>,
        &mut BTreeMap<AssocType, Punctuated<TypeParamBound, Add>>,
        &TokenStream,
    ),
) -> (
    Vec<ParsedFunc>,
    ParsedGenerics,
    (ParsedGenerics, Vec<Ident>, TokenStream),
    TokenStream,
) {
    let mut funcs = vec![];
    let generics = ParsedGenerics::from(&tr.generics);
    let mut trait_type_defs = TokenStream::new();
    let mut types = BTreeMap::new();
    let mut types_c_side_vtbl = BTreeMap::new();

    let mut assocs = BTreeMap::new();

    let trait_name = &tr.ident;

    types.insert(
        Some(AssocType::from(format_ident!("Self"))),
        WrappedType {
            ty: parse2(quote!(CGlueC)).unwrap(),
            // TODO: should we forward ty in here??
            ty_ret_tmp: None,
            ty_static: None,
            return_conv: Some(
                parse2(quote!(|ret| {
                    use #crate_path::from2::From2;
                    CGlueC::from2((ret, cglue_ctx))
                }))
                .expect("Internal closure parsing fail"),
            ),
            lifetime_bound: None,
            lifetime_type_bound: None,
            other_bounds: Some(quote!((CGlueC::ObjType, CGlueCtx): Into<CGlueC>,)),
            other_bounds_simple: Some(quote!((CGlueC::ObjType, CGlueCtx): Into<CGlueC>,)),
            impl_return_conv: Some(quote!(self.build_with_ccont(ret))),
            inject_ret_tmp: false,
            unbounded_hrtb: true,
        },
    );

    let int_result = tr
        .attrs
        .iter()
        .filter(|a| a.path.to_token_stream().to_string() == "int_result")
        .map(|a| {
            a.parse_args::<Ident>()
                .unwrap_or_else(|_| format_ident!("Result"))
        })
        .next();

    // Parse all functions in the trait
    for item in &tr.items {
        match item {
            // We assume types are defined before methods here...
            TraitItem::Type(ty) => process_item(
                (
                    &Some(AssocType::new(ty.ident.clone(), ty.generics.clone())),
                    &ty.bounds,
                    ty.generics.where_clause.as_ref(),
                    &ty.attrs,
                ),
                &tr.ident,
                &generics,
                &mut trait_type_defs,
                &mut types,
                &mut assocs,
                crate_path,
            ),
            TraitItem::Method(m) => {
                let attrs = m
                    .attrs
                    .iter()
                    .map(|a| a.path.to_token_stream().to_string())
                    .collect::<Vec<_>>();

                if attrs.iter().any(|i| i == "skip_func") {
                    continue;
                }

                let custom_impl = m
                    .attrs
                    .iter()
                    .filter(|a| a.path.to_token_stream().to_string() == "custom_impl")
                    .map(|a| a.parse_args::<CustomFuncImpl>().unwrap())
                    .next();

                let only_c_side = m
                    .attrs
                    .iter()
                    .filter(|a| a.path.to_token_stream().to_string() == "vtbl_only")
                    .map(|a| {
                        a.parse_args::<PathTokens>().ok().map(
                            |PathTokens {
                                 lifetime,
                                 path,
                                 tokens,
                             }| {
                                (
                                    lifetime,
                                    Attribute {
                                        pound_token: Default::default(),
                                        style: AttrStyle::Outer,
                                        bracket_token: Default::default(),
                                        path,
                                        tokens,
                                    },
                                )
                            },
                        )
                    })
                    .next();

                let (only_c_side, types) = if let Some(attr) = only_c_side {
                    if !also_parse_vtbl_only {
                        continue;
                    }

                    types_c_side_vtbl.clear();
                    if let Some((lt, attr)) = attr {
                        let mut punctuated = Punctuated::default();

                        if let Some(lt) = lt {
                            punctuated.push_value(TypeParamBound::Lifetime(lt));
                        }

                        let attr_slice = std::slice::from_ref(&attr);
                        process_item(
                            (&None, &punctuated, None, attr_slice),
                            &tr.ident,
                            &generics,
                            &mut trait_type_defs,
                            &mut types_c_side_vtbl,
                            &mut Default::default(),
                            crate_path,
                        );
                    }
                    (true, &types_c_side_vtbl)
                } else {
                    (false, &types)
                };

                let mut iter = m.sig.generics.params.iter();

                if custom_impl.is_none() && iter.any(|p| !matches!(p, GenericParam::Lifetime(_))) {
                    if m.default.is_none() {
                        panic!("Generic function `{}` detected with neither a default nor custom implementation! This is not supported.", m.sig.ident);
                    }
                    continue;
                }

                let int_result_new = m
                    .attrs
                    .iter()
                    .filter(|a| a.path.to_token_stream().to_string() == "int_result")
                    .map(|a| {
                        a.parse_args::<Ident>()
                            .unwrap_or_else(|_| format_ident!("Result"))
                    })
                    .next();

                let int_result = int_result_new.as_ref().or(int_result.as_ref());

                funcs.extend(ParsedFunc::new(
                    m.sig.clone(),
                    trait_name.clone(),
                    &generics,
                    types,
                    int_result,
                    int_result
                        .filter(|_| !attrs.iter().any(|i| i == "no_int_result"))
                        .is_some(),
                    crate_path,
                    only_c_side,
                    custom_impl,
                ));
            }
            _ => {}
        }
    }

    let assoc_types = {
        // CGlueA<X>: bounds, CGlueA<Y>: bounds, ...
        let mut tokens = TokenStream::new();
        // <X> = CGlueA<X>, <Y> = CGlueA<Y>, ...
        let mut equality = TokenStream::new();

        let mut assoc_vec = vec![];

        // TODO: please, make this cleaner without reparsing tokens.
        for (t, mut b) in assocs {
            let t = t.ident;
            let ident = format_ident!("CGlueA{}", t);
            equality.extend(quote!(#t = #ident,));
            if b.is_empty() {
                tokens.extend(quote!(#ident,));
            } else {
                if !b.trailing_punct() {
                    b.push_punct(Default::default());
                }
                tokens.extend(quote!(#ident: #b))
            }
            assoc_vec.push(t);
        }

        (parse2(quote!(<#tokens>)).unwrap(), assoc_vec, equality)
    };

    (funcs, generics, assoc_types, trait_type_defs)
}

pub fn gen_trait(mut tr: ItemTrait, ext_name: Option<&Ident>) -> TokenStream {
    // Path to trait group import.
    let crate_path = crate::util::crate_path();
    let trg_path: TokenStream = quote!(#crate_path::trait_group);

    // Need to preserve the same visibility as the trait itself.
    let vis = tr.vis.to_token_stream();

    let unsafety = tr.unsafety;
    let trait_name = tr.ident.clone();
    let trait_name = &trait_name;

    let trait_impl_name = ext_name.unwrap_or(trait_name);

    let c_void = crate::util::void_type();

    // Additional identifiers
    let vtbl_ident = format_ident!("{}Vtbl", trait_name);
    let vtbl_get_ident = format_ident!("{}VtblGet", trait_name);
    let ret_tmp_ident = format_ident!("{}RetTmp", trait_name);
    let ret_tmp_ident_phantom = format_ident!("{}RetTmpPhantom", trait_name);
    let accessor_trait_ident = format_ident!("{}OpaqueObj", trait_name);
    let assoc_bind_ident = format_ident!("{}AssocBind", trait_name);

    let base_box_trait_obj_ident = format_ident!("{}BaseBox", trait_name);
    let base_ctx_trait_obj_ident = format_ident!("{}BaseCtxBox", trait_name);
    let base_arc_trait_obj_ident = format_ident!("{}BaseArcBox", trait_name);
    let base_mut_trait_obj_ident = format_ident!("{}BaseMut", trait_name);
    let base_ctx_mut_trait_obj_ident = format_ident!("{}BaseCtxMut", trait_name);
    let base_arc_mut_trait_obj_ident = format_ident!("{}BaseArcMut", trait_name);
    let base_ref_trait_obj_ident = format_ident!("{}BaseRef", trait_name);
    let base_ctx_ref_trait_obj_ident = format_ident!("{}BaseCtxRef", trait_name);
    let base_arc_ref_trait_obj_ident = format_ident!("{}BaseArcRef", trait_name);
    let base_trait_obj_ident = format_ident!("{}Base", trait_name);

    let opaque_box_trait_obj_ident = format_ident!("{}Box", trait_name);
    let opaque_ctx_trait_obj_ident = format_ident!("{}CtxBox", trait_name);
    let opaque_arc_trait_obj_ident = format_ident!("{}ArcBox", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("{}Mut", trait_name);
    let opaque_ctx_mut_trait_obj_ident = format_ident!("{}CtxMut", trait_name);
    let opaque_arc_mut_trait_obj_ident = format_ident!("{}ArcMut", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("{}Ref", trait_name);
    let opaque_ctx_ref_trait_obj_ident = format_ident!("{}CtxRef", trait_name);
    let opaque_arc_ref_trait_obj_ident = format_ident!("{}ArcRef", trait_name);

    let (funcs, generics, (assocs, assoc_idents, assoc_equality), trait_type_defs) =
        parse_trait(&tr, &crate_path, true, process_item);

    let cglue_c_opaque_bound = cglue_c_opaque_bound();
    let ctx_bound = ctx_bound();

    tr.ident = trait_impl_name.clone();

    let mut trait_type_bounds = TokenStream::new();

    let ParsedGenerics {
        life_declare,
        life_use,
        gen_declare,
        gen_use,
        gen_where_bounds,
        ..
    } = &generics;

    let ParsedGenerics {
        gen_use: assoc_use,
        gen_declare: assoc_declare,
        ..
    } = &assocs;

    let assoc_declare_stripped = assocs.declare_without_nonstatic_bounds();

    // TODO: Is there a better solution?
    let cglue_a_outlives = if life_use.is_empty() {
        None
    } else {
        let mut outlives = quote!(:);
        for lt in life_use.iter() {
            outlives.extend(quote!(#lt + ));
        }
        Some(outlives)
    };

    let gen_declare_stripped = generics.declare_without_nonstatic_bounds();
    let gen_lt_bounds = generics.declare_lt_for_all(&quote!('cglue_a));
    let gen_sabi_bounds = generics.declare_sabi_for_all(&crate_path);
    let assoc_sabi_bounds = assocs.declare_sabi_for_all(&crate_path);

    let gen_where_bounds_base_nolt = gen_where_bounds.clone();

    let gen_where_bounds_base = quote! {
        #gen_where_bounds
        #gen_lt_bounds
    };

    let gen_where_bounds = quote! {
        #gen_where_bounds_base
        #gen_sabi_bounds
        #assoc_sabi_bounds
    };

    #[cfg(feature = "layout_checks")]
    let derive_layouts = quote!(#[derive(::abi_stable::StableAbi)]);
    #[cfg(not(feature = "layout_checks"))]
    let derive_layouts = quote!();

    // Function definitions in the vtable
    let mut vtbl_func_definitions = TokenStream::new();

    for func in &funcs {
        func.vtbl_def(&mut vtbl_func_definitions);
    }

    // Getters for vtable functions
    let mut vtbl_getter_defintions = TokenStream::new();

    for func in &funcs {
        func.vtbl_getter_def(&mut vtbl_getter_defintions);
    }

    // Default functions for vtable reference
    let mut vtbl_default_funcs = TokenStream::new();

    for func in &funcs {
        func.vtbl_default_def(&mut vtbl_default_funcs);
    }

    // Define wrapped functions for the vtable
    let mut cfuncs = TokenStream::new();

    let ret_tmp_ty = quote!(#ret_tmp_ident<CGlueCtx, #gen_use #assoc_use>);

    for func in funcs.iter() {
        let extra_bounds = func.cfunc_def(
            &mut cfuncs,
            &trg_path,
            &ret_tmp_ty,
            &assocs,
            &assoc_equality,
        );
        trait_type_bounds.extend(extra_bounds.to_token_stream());
    }

    // Define wrapped temp storage
    let mut ret_tmp_type_defs = TokenStream::new();

    for func in funcs.iter() {
        func.ret_tmp_def(&mut ret_tmp_type_defs);
    }

    // Define Default calls for temp storage
    let mut ret_tmp_default_defs = TokenStream::new();

    for func in funcs.iter() {
        func.ret_default_def(&mut ret_tmp_default_defs);
    }

    // Define Default calls for temp storage
    let mut ret_tmp_getter_defs = TokenStream::new();

    for func in funcs.iter() {
        func.ret_getter_def(&mut ret_tmp_getter_defs);
    }

    // Phantom data for temp storage including all types, because we do not
    // actually know the types used inside the temp storage.
    // I mean we could cross filter it but it's a little bit much work.
    let phantom_data_definitions = generics.phantom_data_definitions();
    let phantom_data_init = generics.phantom_data_init();
    let assoc_phantom_data_definitions = assocs.phantom_data_definitions();
    let assoc_phantom_data_init = assocs.phantom_data_init();

    // Implement the trait for a type that has CGlueObj<OpaqueCGlueVtblT, RetTmp>
    let mut trait_impl_fns = TokenStream::new();

    // TODO: clean this up
    let mut need_mut = false;
    let mut need_own = false;
    let mut need_cgluef = false;
    let mut return_self = false;

    for func in &funcs {
        let (nm, no, rs) = func.trait_impl(&mut trait_impl_fns);
        need_mut = nm || need_mut;
        need_own = no || need_own;
        need_cgluef = !no || need_cgluef;
        return_self = rs || return_self;
    }

    let required_ctx = if need_mut {
        quote!(#trg_path::CGlueObjMut<#ret_tmp_ty, Context = CGlueCtx> + )
    } else {
        quote!(#trg_path::CGlueObjRef<#ret_tmp_ty, Context = CGlueCtx> + )
    };

    let cglue_c_into_inner = if need_own {
        Some(quote!(
            CGlueC::InstType: #trg_path::IntoInner<InnerTarget = CGlueC::ObjType>,
        ))
    } else {
        None
    };

    let cglue_c_bounds = quote!(: #required_ctx 'cglue_a);

    // Add supertrait bounds here. Note that it probably won't work, unless they are one of the
    // core traits (Send, etc.).
    let supertrait_bounds = {
        let sup = &tr.supertraits;
        if sup.is_empty() {
            quote!()
        } else {
            quote!(+ #sup)
        }
    };

    // If wrapping an external trait, generate an internal implementation for CGlueTraitObj

    let internal_trait_impl = if let Some(ext_name) = ext_name {
        let mut impls = TokenStream::new();

        for a in &assoc_idents {
            impls.extend(quote!(type #a = <Self as #ext_name<#life_use #gen_use>>::#a;));
        }

        for func in &funcs {
            func.int_trait_impl(None, ext_name, &mut impls);
        }

        quote! {
            #unsafety impl<#life_declare CGlueT, CGlueV, CGlueC, CGlueR, #gen_declare> #trait_name<#life_use #gen_use>
                for #trg_path::CGlueTraitObj<'_, CGlueT, CGlueV, CGlueC, CGlueR>
            where
                Self: #ext_name<#life_use #gen_use>
            {
                #impls
            }
        }
    } else {
        quote!()
    };

    // Formatted documentation strings
    let vtbl_doc = format!(" CGlue vtable for trait {}.", trait_name);

    let base_box_trait_obj_doc = format!(" Boxed CGlue trait object for trait {}.", trait_name);
    let base_ctx_trait_obj_doc = format!(
        " CtxBoxed CGlue trait object for trait {} with context.",
        trait_name
    );
    let base_arc_trait_obj_doc = format!(" Boxed CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.", trait_name);
    let base_mut_trait_obj_doc = format!(" By-mut CGlue trait object for trait {}.", trait_name);
    let base_ctx_mut_trait_obj_doc = format!(
        " By-mut CGlue trait object for trait {} with a context.",
        trait_name
    );
    let base_arc_mut_trait_obj_doc = format!(" By-mut CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.", trait_name);
    let base_ref_trait_obj_doc = format!(" By-ref CGlue trait object for trait {}.", trait_name);
    let base_ctx_ref_trait_obj_doc = format!(
        " By-ref CGlue trait object for trait {} with a context.",
        trait_name
    );
    let base_arc_ref_trait_obj_doc = format!(" By-ref CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.", trait_name);
    let base_trait_obj_doc = format!(" Base CGlue trait object for trait {}.", trait_name);
    let opaque_box_trait_obj_doc =
        format!(" Opaque Boxed CGlue trait object for trait {}.", trait_name);
    let opaque_ctx_trait_obj_doc = format!(
        " Opaque CtxBoxed CGlue trait object for trait {} with a context.",
        trait_name
    );
    let opaque_arc_trait_obj_doc = format!(" Opaque Boxed CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.", trait_name);
    let opaque_mut_trait_obj_doc = format!(
        " Opaque by-mut CGlue trait object for trait {}.",
        trait_name
    );
    let opaque_ctx_mut_trait_obj_doc = format!(
        " Opaque by-mut CGlue trait object for trait {} with a context.",
        trait_name
    );
    let opaque_arc_mut_trait_obj_doc = format!(
        " Opaque by-mut CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.",
        trait_name
    );
    let opaque_ref_trait_obj_doc = format!(
        " Opaque by-ref CGlue trait object for trait {}.",
        trait_name
    );
    let opaque_ctx_ref_trait_obj_doc = format!(
        " Opaque by-ref CGlue trait object for trait {} with a context.",
        trait_name
    );
    let opaque_arc_ref_trait_obj_doc = format!(
        " Opaque by-ref CGlue trait object for trait {} with a [`CArc`](cglue::arc::CArc) reference counted context.",
        trait_name
    );
    let submod_name = format_ident!("cglue_{}", trait_name.to_string().to_lowercase());

    let ret_tmp = if !ret_tmp_type_defs.is_empty() {
        quote! {
            /// Temporary return value structure, for returning wrapped references.
            ///
            /// This structure contains data for each vtable function that returns a reference to
            /// an associated type. Note that these temporary values should not be accessed
            /// directly. Use the trait functions.
            #[repr(C)]
            #derive_layouts
            pub struct #ret_tmp_ident<CGlueCtx: #ctx_bound, #gen_use #assoc_use>
            {
                #ret_tmp_type_defs
                #phantom_data_definitions
                #assoc_phantom_data_definitions
                _ty_cglue_ctx: ::core::marker::PhantomData<CGlueCtx>,
            }

            impl<CGlueCtx: #ctx_bound, #gen_use #assoc_use> #ret_tmp_ident<CGlueCtx, #gen_use #assoc_use>
            {
                #ret_tmp_getter_defs
            }

            impl<CGlueCtx: #ctx_bound, #gen_use #assoc_use> Default for #ret_tmp_ident<CGlueCtx, #gen_use #assoc_use>
            {
                fn default() -> Self {
                    Self {
                        #ret_tmp_default_defs
                        #phantom_data_init
                        #assoc_phantom_data_init
                        _ty_cglue_ctx: ::core::marker::PhantomData,
                    }
                }
            }
        }
    } else {
        quote! {
            /// Technically unused phantom data definition structure.
            #[repr(C)]
            #derive_layouts
            pub struct #ret_tmp_ident_phantom<CGlueCtx: #ctx_bound, #gen_use #assoc_use>
            {
                #phantom_data_definitions
                #assoc_phantom_data_definitions
                _ty_cglue_ctx: ::core::marker::PhantomData<CGlueCtx>,
            }

            /// Type definition for temporary return value wrapping storage.
            ///
            /// The trait does not use return wrapping, thus is a typedef to `PhantomData`.
            ///
            /// Note that `cbindgen` will generate wrong structures for this type. It is important
            /// to go inside the generated headers and fix it - all RetTmp structures without a
            /// body should be completely deleted, both as types, and as fields in the
            /// groups/objects. If C++11 templates are generated, it is important to define a
            /// custom type for CGlueTraitObj that does not have `ret_tmp` defined, and change all
            /// type aliases of this trait to use that particular structure.
            pub type #ret_tmp_ident<CGlueCtx, #gen_use #assoc_use> = ::core::marker::PhantomData<#ret_tmp_ident_phantom<CGlueCtx, #gen_use #assoc_use>>;
        }
    };

    #[cfg(feature = "layout_checks")]
    let (opaque_vtbl_bounds, container_vtbl_bounds) = (
        quote!(#vtbl_ident<'cglue_a, CGlueC::OpaqueTarget, #gen_use #assoc_use>: ::abi_stable::StableAbi,),
        quote!(#vtbl_ident<'cglue_a, <Self as #trg_path::GetContainer>::ContType, #gen_use #assoc_use>: ::abi_stable::StableAbi,),
    );
    #[cfg(not(feature = "layout_checks"))]
    let (opaque_vtbl_bounds, container_vtbl_bounds) = (quote!(), quote!());

    #[cfg(feature = "layout_checks")]
    let (layout_checkable_bound, objcont_accessor_bound) = (
        quote!(::abi_stable::StableAbi),
        quote!(<CGlueO as #trg_path::GetContainer>::ContType: ::abi_stable::StableAbi,),
    );
    #[cfg(not(feature = "layout_checks"))]
    let (layout_checkable_bound, objcont_accessor_bound) = (quote!(), quote!());

    // Glue it all together
    quote! {
        #tr

        #[doc(hidden)]
        #vis use #submod_name::*;

        pub mod #submod_name {
            use super::*;
            use super::#trait_impl_name;

            #vis use cglue_internal::{
                #vtbl_ident,
                #vtbl_get_ident,
                #ret_tmp_ident,
                #accessor_trait_ident,
                #assoc_bind_ident,

                #base_box_trait_obj_ident,
                #base_ctx_trait_obj_ident,
                #base_arc_trait_obj_ident,
                #base_mut_trait_obj_ident,
                #base_ctx_mut_trait_obj_ident,
                #base_arc_mut_trait_obj_ident,
                #base_ref_trait_obj_ident,
                #base_ctx_ref_trait_obj_ident,
                #base_arc_ref_trait_obj_ident,
                #base_trait_obj_ident,

                #opaque_box_trait_obj_ident,
                #opaque_ctx_trait_obj_ident,
                #opaque_arc_trait_obj_ident,
                #opaque_mut_trait_obj_ident,
                #opaque_ctx_mut_trait_obj_ident,
                #opaque_arc_mut_trait_obj_ident,
                #opaque_ref_trait_obj_ident,
                #opaque_ctx_ref_trait_obj_ident,
                #opaque_arc_ref_trait_obj_ident,
            };

            mod cglue_internal {
            use super::*;
            use super::#trait_impl_name;

            /* Primary vtable definition. */

            #[doc = #vtbl_doc]
            ///
            /// This virtual function table contains ABI-safe interface for the given trait.
            #[repr(C)]
            #derive_layouts
            pub struct #vtbl_ident<
                'cglue_a,
                CGlueC: 'cglue_a + #trg_path::CGlueObjBase,
                #gen_declare_stripped
                #assoc_declare_stripped
            >
            where
                #gen_where_bounds_base_nolt
            {
                #vtbl_func_definitions
                #assoc_phantom_data_definitions
                _lt_cglue_a: ::core::marker::PhantomData<&'cglue_a CGlueC>,
            }

            impl<
                'cglue_a,
                CGlueC: #trg_path::CGlueObjBase,
                #gen_declare_stripped
                #assoc_declare_stripped
            > #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>
            where
                #gen_where_bounds
            {
                #vtbl_getter_defintions
            }

            #ret_tmp

            /* Default implementation. */

            /// Default vtable reference creation.
            impl<'cglue_a, CGlueC #cglue_c_bounds, CGlueCtx: #ctx_bound, #gen_declare_stripped #assoc_declare_stripped> Default
                for &'cglue_a #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>
            where #gen_where_bounds #trait_type_bounds #cglue_c_into_inner
                CGlueC::ObjType: for<#life_declare> #trait_name<#life_use #gen_use #assoc_equality>,
                CGlueC: #trg_path::Opaquable,
                CGlueC::OpaqueTarget: #trg_path::GenericTypeBounds,
                #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>: #trg_path::CGlueBaseVtbl,
            {
                /// Create a static vtable for the given type.
                fn default() -> Self {
                    &#vtbl_ident {
                        #vtbl_default_funcs
                        #assoc_phantom_data_init
                        _lt_cglue_a: ::core::marker::PhantomData,
                    }
                }
            }

            /* Vtable trait implementations. */

            impl<
                'cglue_a,
                CGlueC: #trg_path::CGlueObjBase,
                #gen_declare_stripped
                #assoc_declare_stripped
            >
                #trg_path::CGlueVtblCont for #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>
            where
                #gen_where_bounds
            {
                type ContType = CGlueC;
            }

            unsafe impl<
                'cglue_a,
                CGlueC: #trg_path::Opaquable + #trg_path::CGlueObjBase + 'cglue_a,
                #gen_declare_stripped
                #assoc_declare_stripped
            > #trg_path::CGlueBaseVtbl
                for #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>
            where #gen_where_bounds #cglue_c_opaque_bound
                CGlueC::ObjType: for<#life_declare> #trait_name<#life_use #gen_use #assoc_equality>,
                CGlueC::OpaqueTarget: #trg_path::GenericTypeBounds,
                #opaque_vtbl_bounds
            {
                type OpaqueVtbl = #vtbl_ident<'cglue_a, CGlueC::OpaqueTarget, #gen_use #assoc_use>;
                type Context = CGlueC::Context;
                type RetTmp = #ret_tmp_ident<CGlueC::Context, #gen_use #assoc_use>;
            }

            impl<'cglue_a, CGlueC #cglue_c_bounds, CGlueCtx: #ctx_bound, #gen_declare_stripped #assoc_declare_stripped> #trg_path::CGlueVtbl<CGlueC>
                for #vtbl_ident<'cglue_a, CGlueC, #gen_use #assoc_use>
            where #gen_where_bounds
                  #cglue_c_opaque_bound
                CGlueC: #trg_path::Opaquable,
                CGlueC::OpaqueTarget: #trg_path::GenericTypeBounds,
                CGlueC::ObjType: for<#life_declare> #trait_name<#life_use #gen_use #assoc_equality> {}

            #[doc = #base_box_trait_obj_doc]
            pub type #base_box_trait_obj_ident<'cglue_a, CGlueT, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, #trg_path::NoContext, #gen_use #assoc_use>;

            #[doc = #base_ctx_trait_obj_doc]
            pub type #base_ctx_trait_obj_ident<'cglue_a, CGlueT, CGlueCtx, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #base_arc_trait_obj_doc]
            pub type #base_arc_trait_obj_ident<'cglue_a, CGlueT, CGlueC, #gen_use #assoc_use>
                = #base_ctx_trait_obj_ident<'cglue_a, CGlueT, #crate_path::arc::CArc<CGlueC>, #gen_use #assoc_use>;

            #[doc = #base_mut_trait_obj_doc]
            pub type #base_mut_trait_obj_ident<'cglue_a, CGlueT, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a mut CGlueT, #trg_path::NoContext, #gen_use #assoc_use>;

            #[doc = #base_ctx_mut_trait_obj_doc]
            pub type #base_ctx_mut_trait_obj_ident<'cglue_a, CGlueT, CGlueCtx, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a mut CGlueT, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #base_arc_mut_trait_obj_doc]
            pub type #base_arc_mut_trait_obj_ident<'cglue_a, CGlueT, CGlueC, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a mut CGlueT, #crate_path::arc::CArc<CGlueC>, #gen_use #assoc_use>;

            #[doc = #base_ref_trait_obj_doc]
            pub type #base_ref_trait_obj_ident<'cglue_a, CGlueT, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a CGlueT, #trg_path::NoContext, #gen_use #assoc_use>;

            #[doc = #base_ctx_ref_trait_obj_doc]
            pub type #base_ctx_ref_trait_obj_ident<'cglue_a, CGlueT, CGlueCtx, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a CGlueT, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #base_arc_ref_trait_obj_doc]
            pub type #base_arc_ref_trait_obj_ident<'cglue_a, CGlueT, CGlueC, #gen_use #assoc_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a CGlueT, #crate_path::arc::CArc<CGlueC>, #gen_use #assoc_use>;

            #[doc = #base_trait_obj_doc]
            pub type #base_trait_obj_ident<'cglue_a, CGlueInst, CGlueCtx, #gen_use #assoc_use>
                = #trg_path::CGlueTraitObj::<
                    'cglue_a,
                    CGlueInst,
                    #vtbl_ident<
                        'cglue_a,
                        #trg_path::CGlueObjContainer<CGlueInst, CGlueCtx, #ret_tmp_ident<CGlueCtx, #gen_use #assoc_use>>,
                        #gen_use
                        #assoc_use
                    >,
                    CGlueCtx,
                    #ret_tmp_ident<CGlueCtx, #gen_use #assoc_use>
                >;

            #[doc = #opaque_box_trait_obj_doc]
            pub type #opaque_box_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_box_trait_obj_ident<'cglue_a, #c_void, #gen_use #assoc_use>;

            #[doc = #opaque_ctx_trait_obj_doc]
            pub type #opaque_ctx_trait_obj_ident<'cglue_a, CGlueCtx, #gen_use #assoc_use>
                = #base_ctx_trait_obj_ident<'cglue_a, #c_void, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #opaque_arc_trait_obj_doc]
            pub type #opaque_arc_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_arc_trait_obj_ident<'cglue_a, #c_void, #c_void, #gen_use #assoc_use>;

            #[doc = #opaque_mut_trait_obj_doc]
            pub type #opaque_mut_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_mut_trait_obj_ident<'cglue_a, #c_void, #gen_use #assoc_use>;

            #[doc = #opaque_ctx_mut_trait_obj_doc]
            pub type #opaque_ctx_mut_trait_obj_ident<'cglue_a, CGlueCtx, #gen_use #assoc_use>
                = #base_ctx_mut_trait_obj_ident<'cglue_a, #c_void, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #opaque_arc_mut_trait_obj_doc]
            pub type #opaque_arc_mut_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_arc_mut_trait_obj_ident<'cglue_a, #c_void, #c_void, #gen_use #assoc_use>;

            #[doc = #opaque_ref_trait_obj_doc]
            pub type #opaque_ref_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_ref_trait_obj_ident<'cglue_a, #c_void, #gen_use #assoc_use>;

            #[doc = #opaque_ctx_ref_trait_obj_doc]
            pub type #opaque_ctx_ref_trait_obj_ident<'cglue_a, CGlueCtx, #gen_use #assoc_use>
                = #base_ctx_ref_trait_obj_ident<'cglue_a, #c_void, CGlueCtx, #gen_use #assoc_use>;

            #[doc = #opaque_arc_ref_trait_obj_doc]
            pub type #opaque_arc_ref_trait_obj_ident<'cglue_a, #gen_use #assoc_use>
                = #base_arc_ref_trait_obj_ident<'cglue_a, #c_void, #c_void, #gen_use #assoc_use>;

            /* Internal wrapper functions. */

            #cfuncs

            /* Define trait for simpler type accesses */

            pub trait #accessor_trait_ident<'cglue_a #cglue_a_outlives, #life_declare #gen_declare #assoc_declare>
                : 'cglue_a + #trg_path::GetContainer + #vtbl_get_ident<'cglue_a, #gen_use #assoc_use> #supertrait_bounds
            where
                #gen_where_bounds
            {
                type #vtbl_ident: #trg_path::CGlueVtblCont<ContType = <Self as #trg_path::GetContainer>::ContType> + #layout_checkable_bound;
            }

            impl<'cglue_a #cglue_a_outlives, #life_declare
                CGlueO: 'cglue_a + #trg_path::GetContainer + #vtbl_get_ident<'cglue_a, #gen_use #assoc_use> #supertrait_bounds, #gen_declare #assoc_declare>
                #accessor_trait_ident<'cglue_a, #life_use #gen_use #assoc_use> for CGlueO
            where
                #objcont_accessor_bound
                #gen_where_bounds
            {
                type #vtbl_ident = #vtbl_ident<'cglue_a, <Self as #trg_path::GetContainer>::ContType, #gen_use #assoc_use>;
            }

            /* Binds associated types for a given trait. */

            pub trait #assoc_bind_ident<#gen_use> {
                type Assocs;
            }

            impl<
                'cglue_a,
                CGlueT: ::core::ops::Deref<Target = CGlueF>,
                CGlueF,
                CGlueCtx: #ctx_bound,
                CGlueRetTmp,
                #gen_declare_stripped
                #assoc_declare_stripped
            >
                #assoc_bind_ident<#gen_use>
                for #trg_path::CGlueTraitObj<'cglue_a, CGlueT, #vtbl_ident<'cglue_a, #trg_path::CGlueObjContainer<CGlueT, CGlueCtx, CGlueRetTmp>, #gen_use #assoc_use>, CGlueCtx, CGlueRetTmp>
            where
                #gen_where_bounds_base
            {
                type Assocs = (#assoc_use);
            }

            /* Getters for vtables. Automatically implemented for CGlueTraitObj */

            pub trait #vtbl_get_ident<'cglue_a, #gen_declare_stripped #assoc_declare_stripped>:
                #trg_path::GetContainer
            where
                #gen_where_bounds_base
            {
                fn get_vtbl(&self) -> &#vtbl_ident<'cglue_a, <Self as #trg_path::GetContainer>::ContType, #gen_use #assoc_use>;
            }

            impl<
                'cglue_a,
                CGlueT: ::core::ops::Deref<Target = CGlueF>,
                CGlueF,
                CGlueCtx: #ctx_bound,
                CGlueRetTmp,
                #gen_declare_stripped
                #assoc_declare_stripped
            >
                #vtbl_get_ident<'cglue_a, #gen_use #assoc_use>
                for #trg_path::CGlueTraitObj<'cglue_a, CGlueT, #vtbl_ident<'cglue_a, #trg_path::CGlueObjContainer<CGlueT, CGlueCtx, CGlueRetTmp>, #gen_use #assoc_use>, CGlueCtx, CGlueRetTmp>
            where
                #gen_where_bounds_base
            {
                fn get_vtbl(&self) -> &#vtbl_ident<'cglue_a, <Self as #trg_path::GetContainer>::ContType, #gen_use #assoc_use> {
                    #trg_path::GetVtblBase::get_vtbl_base(self)
                }
            }

            /* Trait implementation. */

            #unsafety impl<'cglue_a #cglue_a_outlives, #life_declare
                CGlueO: 'cglue_a + #vtbl_get_ident<'cglue_a, #gen_use #assoc_use> #supertrait_bounds
                    // We essentially need only this bound, but we repeat the previous ones because
                    // otherwise we get conflicting impl errors.
                    // TODO: Is this a bug? Typically Rust typesystem doesn't complain in such cases.
                    + #accessor_trait_ident<'cglue_a, #life_use #gen_use #assoc_use>
                    // Same here.
                    + #assoc_bind_ident<#gen_use Assocs = (#assoc_use)>
                    // We also need to specify this one, for some reason. If not for conflicting
                    // impl errors, `GetVtblBase` would be a relatively redundant trait with no
                    // purpose.
                    + #trg_path::GetVtblBase<#vtbl_ident<'cglue_a, <Self as #trg_path::GetContainer>::ContType, #gen_use #assoc_use>>,
            #gen_declare #assoc_declare>
                #trait_impl_name<#life_use #gen_use> for CGlueO
            where
                #gen_where_bounds
                #container_vtbl_bounds
            {
                // TODO: #assoc_type_def
                #trait_type_defs
                #trait_impl_fns
            }

            #internal_trait_impl
            }
        }
    }
}
