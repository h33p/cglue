use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::{ParsedFunc, WrappedType};
use super::generics::{GenericType, ParsedGenerics};

use quote::*;
use syn::*;

pub fn ctx_bound() -> TokenStream {
    let crate_path = crate::util::crate_path();
    quote!(#crate_path::trait_group::ContextRef<Context = CGlueC> + )
}

// TODO: Add dynamic setting of Send / Sync
pub fn cd_bounds() -> TokenStream {
    quote!('static + Clone + Send + Sync)
}

pub fn cd_opaque_bound() -> TokenStream {
    let crate_path = crate::util::crate_path();
    quote!(#crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>)
}

pub fn cglue_c_opaque_bound() -> TokenStream {
    let crate_path = crate::util::crate_path();
    quote!(CGlueC::OpaqueTarget: #crate_path::trait_group::Opaquable,)
}

pub fn process_item(
    ty: &TraitItemType,
    trait_name: &Ident,
    generics: &ParsedGenerics,
    trait_type_defs: &mut TokenStream,
    types: &mut BTreeMap<Ident, WrappedType>,
    crate_path: &TokenStream,
) {
    let c_void = quote!(::core::ffi::c_void);

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

    let mut lifetime_bounds = ty.bounds.iter().filter_map(|b| match b {
        TypeParamBound::Lifetime(lt) => Some(lt),
        _ => None,
    });

    let orig_lifetime_bound = lifetime_bounds.next();

    if lifetime_bounds.next().is_some() {
        panic!("Traits with multiple lifetime bounds are not supported!");
    }

    for attr in &ty.attrs {
        let s = attr.path.to_token_stream().to_string();

        let x = s.as_str();

        match x {
            "wrap_with" => {
                let new_ty = attr
                    .parse_args::<GenericType>()
                    .expect("Invalid type in wrap_with.");

                let ty_ident = &ty.ident;

                trait_type_defs.extend(quote!(type #ty_ident = #new_ty;));

                types.insert(
                    ty_ident.clone(),
                    WrappedType {
                        ty: new_ty,
                        ty_static: None,
                        return_conv: None,
                        lifetime_bound: None,
                        lifetime_type_bound: None,
                        other_bounds: None,
                        other_bounds_simple: None,
                        impl_return_conv: None,
                        inject_ret_tmp: false,
                        unbounded_hrtb: false,
                        needs_ctx: false,
                    },
                );
            }
            "return_wrap" => {
                let closure = attr
                    .parse_args::<ExprClosure>()
                    .expect("A valid closure must be supplied accepting the wrapped type!");

                types
                    .get_mut(&ty.ident)
                    .expect("Type must be first wrapped with #[wrap_with(T)] atribute.")
                    .return_conv = Some(closure);
            }
            "wrap_with_obj"
            | "wrap_with_obj_ref"
            | "wrap_with_obj_mut"
            | "wrap_with_group"
            | "wrap_with_group_ref"
            | "wrap_with_group_mut" => {
                let mut new_ty = attr
                    .parse_args::<GenericType>()
                    .expect("Invalid type in wrap_with.");

                let target = new_ty.target.clone();

                if ["wrap_with_obj", "wrap_with_obj_ref", "wrap_with_obj_mut"].contains(&x) {
                    new_ty.target = format_ident!("{}Base", target.to_string()).to_token_stream();
                }

                // These variables model a `CGlueF::#ty_ident: Into<SomeGroup>` bound.
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

                let ty_ident = &ty.ident;

                let gen_use = &generics.gen_use;

                let (type_bounds, type_bounds_simple, needs_ctx) = {
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

                    let cglue_f_ty_ident =
                        quote!(<CGlueF as #trait_name<#hrtb_lifetime_use #gen_use>>::#ty_ident);

                    let cglue_f_ty_simple_ident =
                        quote!(<CGlueF as #trait_name<#simple_lifetime_use #gen_use>>::#ty_ident);

                    let mut new_ty_hrtb = from_new_ty.clone();
                    let mut new_ty_simple = from_new_ty_simple.clone();

                    let needs_ctx = if x == "wrap_with_group" || x == "wrap_with_obj" {
                        new_ty.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<#lifetime, #c_void, CGlueD>, #c_void, CGlueD, CGlueD,),
                        );
                        new_ty_hrtb.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<#from_lifetime, #c_void, CGlueD>, #c_void, CGlueD, CGlueD,),
                        );
                        new_ty_simple.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<#from_lifetime_simple, #c_void, CGlueD>, #c_void, CGlueD, CGlueD,),
                        );
                        new_ty_static.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<'static, #c_void, CGlueD>, #c_void, CGlueD, CGlueD,),
                        );
                        from_new_ty.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<#from_lifetime, #cglue_f_ty_ident, CGlueC>, #cglue_f_ty_ident, CGlueC, CGlueD,),
                        );
                        from_new_ty_simple.push_types_start(
                            quote!(#crate_path::boxed::CtxBox<#from_lifetime_simple, #cglue_f_ty_simple_ident, CGlueC>, #cglue_f_ty_simple_ident, CGlueC, CGlueD,),
                        );

                        true
                    } else if x == "wrap_with_group_ref" || x == "wrap_with_obj_ref" {
                        let no_context = quote!(#crate_path::trait_group::NoContext);
                        new_ty.push_types_start(
                            quote!(&#lifetime #c_void, #c_void, #no_context, #no_context,),
                        );
                        new_ty_hrtb.push_types_start(
                            quote!(&#from_lifetime #c_void, #c_void, #no_context, #no_context,),
                        );
                        new_ty_simple
                            .push_types_start(quote!(&#from_lifetime_simple #c_void, #c_void, #no_context, #no_context,));
                        new_ty_static.push_types_start(
                            quote!(&'static #c_void, #c_void, #no_context, #no_context,),
                        );
                        from_new_ty.push_types_start(
                            quote!(&#from_lifetime CGlueF::#ty_ident, #cglue_f_ty_ident, #no_context, #no_context,),
                        );
                        from_new_ty_ref.extend(quote!(&#from_lifetime));
                        from_new_ty_simple.push_types_start(
                            quote!(&#from_lifetime_simple CGlueF::#ty_ident, #cglue_f_ty_simple_ident, #no_context, #no_context,),
                        );
                        from_new_ty_simple_ref.extend(quote!(&#from_lifetime_simple));

                        false
                    } else if x == "wrap_with_group_mut" || x == "wrap_with_obj_mut" {
                        let no_context = quote!(#crate_path::trait_group::NoContext);
                        new_ty.push_types_start(
                            quote!(&#lifetime mut #c_void, #c_void, #no_context, #no_context,),
                        );
                        new_ty_hrtb.push_types_start(
                            quote!(&#from_lifetime mut #c_void, #c_void, #no_context, #no_context,),
                        );
                        new_ty_simple
                            .push_types_start(quote!(&#from_lifetime_simple mut #c_void, #c_void, #no_context, #no_context,));
                        new_ty_static.push_types_start(
                            quote!(&'static mut #c_void, #c_void, #no_context, #no_context,),
                        );
                        from_new_ty.push_types_start(
                            quote!(&#from_lifetime mut #cglue_f_ty_ident, #cglue_f_ty_ident, #no_context, #no_context,),
                        );
                        from_new_ty_ref.extend(quote!(&#from_lifetime mut));
                        from_new_ty_simple.push_types_start(
                            quote!(&#from_lifetime_simple mut #cglue_f_ty_simple_ident, #cglue_f_ty_simple_ident, #no_context, #no_context,),
                        );
                        from_new_ty_simple_ref.extend(quote!(&#from_lifetime_simple mut));

                        false
                    } else {
                        unreachable!()
                    };

                    let (type_bounds, type_bounds_simple) = {
                        let (ty_ref, ty_ref_simple) = if needs_ctx {
                            (
                                quote!((#from_new_ty_ref #cglue_f_ty_ident, CGlueC)),
                                quote!((#from_new_ty_simple_ref #cglue_f_ty_simple_ident, CGlueC)),
                            )
                        } else {
                            (
                                quote!(#from_new_ty_ref #cglue_f_ty_ident),
                                quote!(#from_new_ty_simple_ref #cglue_f_ty_simple_ident),
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

                        (type_bounds, type_bounds_simple)
                    };

                    let cd_bounds = cd_bounds();

                    // Inject opaquable to CGlueC's opaque target, to form an infinitely opaquable
                    // chain. Needed when CGlueC is used.
                    let (type_bounds, type_bounds_simple) = if needs_ctx {
                        (
                            quote!(#type_bounds CGlueD: #cd_bounds + #crate_path::trait_group::Opaquable, ),
                            quote!(#type_bounds_simple CGlueD: #cd_bounds + #crate_path::trait_group::Opaquable, ),
                        )
                    } else {
                        (type_bounds, type_bounds_simple)
                    };

                    (type_bounds, type_bounds_simple, needs_ctx)
                };

                trait_type_defs.extend(quote!(type #ty_ident = #new_ty;));

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
                    (
                        quote! {
                            // SAFETY:
                            // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                            // returning reference to the object.
                            unsafe {
                                ret_tmp.as_mut_ptr().write(std::mem::transmute(ret));
                            }
                        },
                        quote!('cglue_a),
                    )
                };

                let (return_conv, inject_ret_tmp) = match x {
                    "wrap_with_obj" => (
                        parse2(quote!(
                            |ret| trait_obj!((ret, cglue_ctx.clone()) as #target)
                        ))
                        .expect("Internal closure parsing fail"),
                        false,
                    ),
                    "wrap_with_group" => (
                        parse2(quote!(
                            |ret| group_obj!((ret, cglue_ctx.clone()) as #target)
                        ))
                        .expect("Internal closure parsing fail"),
                        false,
                    ),
                    "wrap_with_obj_ref" => (
                        parse2(quote!(|ret: &#conv_bound _| {
                            let ret = trait_obj!(ret as #target);
                            // SAFETY:
                            // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                            // returning reference to the object.
                            unsafe {
                                ret_tmp.as_mut_ptr().write(std::mem::transmute(ret))
                            };
                            unsafe { &*ret_tmp.as_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_group_ref" => (
                        parse2(quote!(|ret: &#conv_bound _| {
                            let ret = group_obj!(ret as #target);
                            // SAFETY:
                            // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                            // returning reference to the object.
                            unsafe {
                                ret_tmp.as_mut_ptr().write(std::mem::transmute(ret))
                            };
                            unsafe { &*ret_tmp.as_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_obj_mut" => (
                        parse2(quote!(|ret: &#conv_bound mut _| {
                            let ret = trait_obj!(ret as #target);
                            #ret_write
                            unsafe { &mut *ret_tmp.as_mut_ptr() }
                        }))
                        .expect("Internal closure parsing fail"),
                        true,
                    ),
                    "wrap_with_group_mut" => (
                        parse2(quote!(|ret: &#conv_bound mut _| {
                            let ret = group_obj!(ret as #target);
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
                    ty_ident.clone(),
                    WrappedType {
                        ty: new_ty,
                        ty_static: Some(new_ty_static),
                        return_conv: Some(return_conv),
                        impl_return_conv: None,
                        lifetime_bound,
                        lifetime_type_bound,
                        other_bounds: Some(type_bounds),
                        other_bounds_simple: Some(type_bounds_simple),
                        inject_ret_tmp,
                        unbounded_hrtb: false,
                        needs_ctx,
                    },
                );
            }
            _ => {}
        }
    }
}

pub fn parse_trait(
    tr: &ItemTrait,
    crate_path: &TokenStream,
    mut process_item: impl FnMut(
        &TraitItemType,
        &Ident,
        &ParsedGenerics,
        &mut TokenStream,
        &mut BTreeMap<Ident, WrappedType>,
        &TokenStream,
    ),
) -> (Vec<ParsedFunc>, ParsedGenerics, TokenStream) {
    let mut funcs = vec![];
    let generics = ParsedGenerics::from(&tr.generics);
    let mut trait_type_defs = TokenStream::new();
    let mut types = BTreeMap::new();

    let trait_name = &tr.ident;

    types.insert(
        format_ident!("Self"),
        WrappedType {
            ty: parse2(quote!(CGlueT)).unwrap(),
            ty_static: None,
            return_conv: Some(
                parse2(quote!(|ret| {
                    use #crate_path::from2::From2;
                    CGlueT::from2((ret, cglue_ctx))
                }))
                .expect("Internal closure parsing fail"),
            ),
            lifetime_bound: None,
            lifetime_type_bound: None,
            other_bounds: Some(quote!((CGlueF, CGlueC): Into<CGlueT>,)),
            other_bounds_simple: Some(quote!((CGlueF, CGlueC): Into<CGlueT>,)),
            impl_return_conv: Some(quote! {
                // SAFETY:
                //
                // The C wrapper type checks that the same object gets returned
                // as the vtables inside `self`.
                unsafe { self.cobj_build(ret) }
            }),
            inject_ret_tmp: false,
            unbounded_hrtb: true,
            needs_ctx: true,
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
                ty,
                &tr.ident,
                &generics,
                &mut trait_type_defs,
                &mut types,
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

                let iter = m.sig.generics.params.iter();

                if iter
                    .into_iter()
                    .any(|p| !matches!(p, GenericParam::Lifetime(_)))
                {
                    if m.default.is_none() {
                        panic!("Generic function `{}` detected without a default implementation! This is not supported.", m.sig.ident);
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

                let int_result = int_result_new.as_ref().or_else(|| int_result.as_ref());

                funcs.extend(ParsedFunc::new(
                    m.sig.clone(),
                    trait_name.clone(),
                    &generics,
                    &types,
                    int_result,
                    int_result
                        .filter(|_| !attrs.iter().any(|i| i == "no_int_result"))
                        .is_some(),
                    &crate_path,
                ));
            }
            _ => {}
        }
    }

    (funcs, generics, trait_type_defs)
}

pub fn gen_trait(mut tr: ItemTrait, ext_name: Option<&Ident>) -> TokenStream {
    // Path to trait group import.
    let crate_path = crate::util::crate_path();
    let trg_path: TokenStream = quote!(#crate_path::trait_group);

    // Need to preserve the same visibility as the trait itself.
    let vis = tr.vis.to_token_stream();

    let trait_name = tr.ident.clone();
    let trait_name = &trait_name;

    let trait_impl_name = ext_name.unwrap_or(trait_name);

    let c_void = quote!(::core::ffi::c_void);

    // Additional identifiers
    let vtbl_ident = format_ident!("{}Vtbl", trait_name);
    let ret_tmp_ident = format_ident!("{}RetTmp", trait_name);
    let ret_tmp_ident_phantom = format_ident!("{}RetTmpPhantom", trait_name);
    let opaque_vtbl_ident = format_ident!("{}OpaqueVtbl", trait_name);

    let base_box_trait_obj_ident = format_ident!("{}BaseBox", trait_name);
    let base_ctx_trait_obj_ident = format_ident!("{}BaseCtxBox", trait_name);
    let base2_ctx_trait_obj_ident = format_ident!("{}Base2CtxBox", trait_name);
    let base_no_ctx_trait_obj_ident = format_ident!("{}BaseNoCtxBox", trait_name);
    let base_arc_trait_obj_ident = format_ident!("{}BaseArcBox", trait_name);
    let base2_arc_trait_obj_ident = format_ident!("{}Base2ArcBox", trait_name);
    let base_mut_trait_obj_ident = format_ident!("{}BaseMut", trait_name);
    let base_ref_trait_obj_ident = format_ident!("{}BaseRef", trait_name);
    let base_trait_obj_ident = format_ident!("{}Base", trait_name);

    let opaque_box_trait_obj_ident = format_ident!("{}Box", trait_name);
    let opaque_ctx_trait_obj_ident = format_ident!("{}CtxBox", trait_name);
    let opaque_no_ctx_trait_obj_ident = format_ident!("{}NoCtxBox", trait_name);
    let opaque_arc_trait_obj_ident = format_ident!("{}ArcBox", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("{}Mut", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("{}Ref", trait_name);
    let opaque_trait_obj_ident = format_ident!("{}Any", trait_name);

    let (funcs, generics, trait_type_defs) = parse_trait(&tr, &crate_path, process_item);

    let cglue_c_opaque_bound = cglue_c_opaque_bound();
    let cd_bounds = cd_bounds();
    let cd_opaque_bound = cd_opaque_bound();

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

    let gen_declare_stripped = generics.declare_without_nonstatic_bounds();

    // Function definitions in the vtable
    let mut vtbl_func_defintions = TokenStream::new();

    for func in &funcs {
        func.vtbl_def(&mut vtbl_func_defintions);
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

    let mut needs_ctx = false;

    for func in funcs.iter() {
        let (extra_bounds, nc) = func.cfunc_def(&mut cfuncs, &trg_path);
        trait_type_bounds.extend(extra_bounds.to_token_stream());
        needs_ctx = nc || needs_ctx;
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

    // If no types are wrapped, and feature is not enabled, inject a 1 byte padding.
    if cfg!(feature = "no_empty_retwrap") && ret_tmp_type_defs.is_empty() {
        ret_tmp_type_defs.extend(quote!(_padding: u8,));
        ret_tmp_default_defs.extend(quote!(_padding: 0,));
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

    // Implement the trait for a type that has CGlueObj<OpaqueCGlueVtblT, RetTmp>
    let mut trait_impl_fns = TokenStream::new();

    // TODO: clean this up
    let mut need_mut = false;
    let mut need_own = false;
    let mut need_cgluef = false;
    let need_cgluec = false;
    let mut return_self = false;

    for func in &funcs {
        let (nm, no, rs) = func.trait_impl(&mut trait_impl_fns);
        need_mut = nm || need_mut;
        need_own = no || need_own;
        need_cgluef = !no || need_cgluef;
        return_self = rs || return_self;
    }

    // If the trait has funcs with mutable self, disallow &CGlueF objects.
    let required_mutability = match (need_mut, need_own) {
        (_, true) => quote!(CGlueObjOwned),
        (true, _) => quote!(CGlueObjMut),
        _ => quote!(CGlueObjRef),
    };

    let required_deref = quote!(::core::ops::Deref<Target = CGlueF>+);

    // We must have a DerefMut, but only if ctx bound is in
    let required_deref = if need_mut && needs_ctx {
        quote!(#required_deref ::core::ops::DerefMut + )
    } else {
        required_deref
    };

    let (required_ctx, required_ctx_opaque, cglue_f_in_impl) = if needs_ctx {
        let mut required_ctx = quote!(#trg_path::ContextRef<Context = CGlueC, ObjType = CGlueF> + );
        let mut required_ctx_opaque =
            quote!(#trg_path::ContextRef<Context = CGlueD, ObjType = CGlueF> + );

        if need_own {
            let owned = quote!(#trg_path::ContextOwned + );
            required_ctx.extend(owned.clone());
            required_ctx_opaque.extend(owned);
        };

        if need_mut && needs_ctx {
            let mutable = quote!(#trg_path::ContextMut + );
            required_ctx.extend(mutable.clone());
            required_ctx_opaque.extend(mutable);
        }

        (required_ctx, required_ctx_opaque, quote!(CGlueF,))
    } else {
        (quote!(), quote!(), quote!())
    };

    let (cglue_t_bounds, cglue_t_bounds_opaque) = if need_own {
        (
            quote!(: #required_ctx #required_deref #trg_path::IntoInner<InnerTarget = CGlueF>),
            quote!(: #required_ctx_opaque 'cglue_a + ::core::ops::Deref<Target = #c_void> + #trg_path::IntoInner),
        )
    } else {
        (
            quote!(: #required_ctx #required_deref),
            quote!(: #required_ctx_opaque 'cglue_a + ::core::ops::Deref<Target = #c_void>),
        )
    };

    let return_self_bound = if return_self {
        quote!(+ #trg_path::CGlueObjBuild<#ret_tmp_ident<#life_use #gen_use>>)
    } else {
        quote!()
    };

    // Inject phantom data depending on what kind of Self parameters are not being used.

    let mut vtbl_phantom_def = TokenStream::new();
    let mut vtbl_phantom_init = TokenStream::new();

    if !need_own {
        vtbl_phantom_def.extend(quote!(_phantom_t: ::core::marker::PhantomData<CGlueT>,));
        vtbl_phantom_init.extend(quote!(_phantom_t: ::core::marker::PhantomData{},));
    }

    if !need_cgluef {
        vtbl_phantom_def.extend(quote!(_phantom_f: ::core::marker::PhantomData<CGlueF>,));
        vtbl_phantom_init.extend(quote!(_phantom_f: ::core::marker::PhantomData{},));
    }

    if !need_cgluec {
        vtbl_phantom_def.extend(quote!(_phantom_c: ::core::marker::PhantomData<CGlueC>,));
        vtbl_phantom_init.extend(quote!(_phantom_c: ::core::marker::PhantomData{},));

        vtbl_phantom_def.extend(quote!(_phantom_d: ::core::marker::PhantomData<CGlueD>,));
        vtbl_phantom_init.extend(quote!(_phantom_d: ::core::marker::PhantomData{},));
    }

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

        for func in &funcs {
            func.int_trait_impl(None, ext_name, &mut impls);
        }

        quote! {
            impl<#life_declare CGlueT, CGlueV, CGlueS: Default, #gen_declare> #trait_name<#life_use #gen_use> for #trg_path::CGlueTraitObj<'_, CGlueT, CGlueV, CGlueS> where Self: #ext_name<#life_use #gen_use> {
                #impls
            }
        }
    } else {
        quote!()
    };

    // Formatted documentation strings
    let vtbl_doc = format!(" CGlue vtable for trait {}.", trait_name);
    let vtbl_opaque_doc = format!(" Opaque CGlue vtable for trait {}.", trait_name);

    let base_box_trait_obj_doc = format!(" Boxed CGlue trait object for trait {}.", trait_name);
    let base_ctx_trait_obj_doc = format!(
        " CtxBoxed CGlue trait object for trait {} with context.",
        trait_name
    );
    let base_no_ctx_trait_obj_doc = format!(" CtxBoxed CGlue trait object for trait {} with [`NoContext`](cglue::trait_group::NoContext) context.", trait_name);
    let base_arc_trait_obj_doc = format!(" CtxBoxed CGlue trait object for trait {} with a [`COptArc`](cglue::arc::COptArc) reference counted context.", trait_name);
    let base_mut_trait_obj_doc = format!(" By-mut CGlue trait object for trait {}.", trait_name);
    let base_ref_trait_obj_doc = format!(" By-ref CGlue trait object for trait {}.", trait_name);
    let base_trait_obj_doc = format!(" Base CGlue trait object for trait {}.", trait_name);
    let opaque_box_trait_obj_doc =
        format!(" Opaque Boxed CGlue trait object for trait {}.", trait_name);
    let opaque_ctx_trait_obj_doc = format!(
        " Opaque CtxBoxed CGlue trait object for trait {} with a context.",
        trait_name
    );
    let opaque_no_ctx_trait_obj_doc = format!(" Opaque CtxBoxed CGlue trait object for trait {} with [`NoContext`](cglue::trait_group::NoContext) context.", trait_name);
    let opaque_arc_trait_obj_doc = format!(" Opaque CtxBoxed CGlue trait object for trait {} with a [`COptArc`](cglue::arc::COptArc) reference counted context.", trait_name);
    let opaque_mut_trait_obj_doc = format!(
        " Opaque by-mut CGlue trait object for trait {}.",
        trait_name
    );
    let opaque_ref_trait_obj_doc = format!(
        " Opaque by-ref CGlue trait object for trait {}.",
        trait_name
    );
    let opaque_trait_obj_doc = format!(" Base opaque CGlue trait object for trait {}.", trait_name);

    let submod_name = format_ident!("cglue_{}", trait_name.to_string().to_lowercase());

    let ret_tmp = if !ret_tmp_type_defs.is_empty() {
        quote! {
            /// Temporary return value structure, for returning wrapped references.
            ///
            /// This structure contains data for each vtable function that returns a reference to
            /// an associated type. Note that these temporary values should not be accessed
            /// directly. Use the trait functions.
            #[repr(C)]
            pub struct #ret_tmp_ident<#gen_declare_stripped> {
                #ret_tmp_type_defs
                #phantom_data_definitions
            }

            impl<#gen_declare_stripped> #ret_tmp_ident<#gen_use> {
                #ret_tmp_getter_defs
            }

            impl<#gen_declare_stripped> Default for #ret_tmp_ident<#gen_use> {
                fn default() -> Self {
                    Self {
                        #ret_tmp_default_defs
                        #phantom_data_init
                    }
                }
            }
        }
    } else {
        quote! {
            /// Technically unused phantom data definition structure.
            pub struct #ret_tmp_ident_phantom<#gen_use> {
                #phantom_data_definitions
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
            pub type #ret_tmp_ident<#gen_use> = ::core::marker::PhantomData<#ret_tmp_ident_phantom<#gen_use>>;
        }
    };

    // Glue it all together
    quote! {
        #tr

        #vis use #submod_name::{
            #vtbl_ident,
            #ret_tmp_ident,
            #opaque_vtbl_ident,

            #base_box_trait_obj_ident,
            #base_ctx_trait_obj_ident,
            #base2_ctx_trait_obj_ident,
            #base_no_ctx_trait_obj_ident,
            #base_arc_trait_obj_ident,
            #base2_arc_trait_obj_ident,
            #base_mut_trait_obj_ident,
            #base_ref_trait_obj_ident,
            #base_trait_obj_ident,

            #opaque_box_trait_obj_ident,
            #opaque_ctx_trait_obj_ident,
            #opaque_no_ctx_trait_obj_ident,
            #opaque_arc_trait_obj_ident,
            #opaque_mut_trait_obj_ident,
            #opaque_ref_trait_obj_ident,
            #opaque_trait_obj_ident,
        };

        mod #submod_name {
            use super::*;
            use super::#trait_impl_name;

            /* Primary vtable definition. */

            #[doc = #vtbl_doc]
            ///
            /// This virtual function table contains ABI-safe interface for the given trait.
            #[repr(C)]
            pub struct #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare_stripped> where #gen_where_bounds {
                #vtbl_func_defintions
                #vtbl_phantom_def
                _lt_cglue_a: ::core::marker::PhantomData<&'cglue_a CGlueT>,
            }

            impl<'cglue_a, CGlueT, CGlueF, CGlueC: 'static + #cd_opaque_bound, CGlueD: 'static, #gen_declare_stripped> #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                where #gen_where_bounds
            {
                #vtbl_getter_defintions
            }

            #ret_tmp

            /* Default implementation. */

            /// Default vtable reference creation.
            impl<'cglue_a, CGlueT #cglue_t_bounds, CGlueF: for<#life_declare> #trait_name<#life_use #gen_use>, CGlueC: #cd_bounds + #cd_opaque_bound, CGlueD: #cd_bounds + #cd_opaque_bound, #gen_declare_stripped> Default for &'cglue_a #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where #gen_where_bounds #trait_type_bounds {
                /// Create a static vtable for the given type.
                fn default() -> Self {
                    &#vtbl_ident {
                        #vtbl_default_funcs
                        #vtbl_phantom_init
                        _lt_cglue_a: ::core::marker::PhantomData,
                    }
                }
            }

            /* Vtable trait implementations. */

            #[doc = #vtbl_opaque_doc]
            ///
            /// This virtual function table has type information destroyed, is used in CGlue objects
            /// and trait groups.
            pub type #opaque_vtbl_ident<'cglue_a, CGlueT, CGlueD, #gen_use> = #vtbl_ident<'cglue_a, CGlueT, #c_void, CGlueD, CGlueD, #gen_use>;

            unsafe impl<'cglue_a, CGlueT: #trg_path::Opaquable + 'cglue_a, CGlueF: for<#life_declare> #trait_name<#life_use #gen_use>, CGlueC: #cd_opaque_bound + 'static, CGlueD: 'static, #gen_declare_stripped> #trg_path::CGlueBaseVtbl for #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where #gen_where_bounds #cglue_c_opaque_bound {
                type OpaqueVtbl = #opaque_vtbl_ident<'cglue_a, CGlueT::OpaqueTarget, CGlueD, #gen_use>;
                type RetTmp = #ret_tmp_ident<#gen_use>;
            }

            impl<'cglue_a, CGlueT #cglue_t_bounds, CGlueF: for<#life_declare> #trait_name<#life_use #gen_use>, CGlueC: #cd_bounds + #cd_opaque_bound, CGlueD: #cd_bounds + #cd_opaque_bound, #gen_declare_stripped> #trg_path::CGlueVtbl<CGlueF, CGlueC> for #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where #gen_where_bounds CGlueT: #trg_path::Opaquable, #cglue_c_opaque_bound {}

            #[doc = #base_box_trait_obj_doc]
            pub type #base_box_trait_obj_ident<'cglue_a, CGlueF, #gen_use>
                = #base_trait_obj_ident<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueF>, CGlueF, #trg_path::NoContext, #trg_path::NoContext, #gen_use>;

            #[doc = #base_ctx_trait_obj_doc]
            pub type #base_ctx_trait_obj_ident<'cglue_a, CGlueF, CGlueC, #gen_use>
                = #base2_ctx_trait_obj_ident<'cglue_a, CGlueF, CGlueC, <CGlueC as #crate_path::trait_group::Opaquable>::OpaqueTarget, #gen_use>;

            #[doc = #base_ctx_trait_obj_doc]
            pub type #base2_ctx_trait_obj_ident<'cglue_a, CGlueF, CGlueC, CGlueD, #gen_use>
                = #base_trait_obj_ident<'cglue_a, #crate_path::boxed::CtxBox<'cglue_a, CGlueF, CGlueC>, CGlueF, CGlueC, CGlueD, #gen_use>;

            #[doc = #base_no_ctx_trait_obj_doc]
            pub type #base_no_ctx_trait_obj_ident<'cglue_a, CGlueF, #gen_use>
                = #base2_ctx_trait_obj_ident<'cglue_a, CGlueF, #trg_path::NoContext, #trg_path::NoContext, #gen_use>;

            #[doc = #base_arc_trait_obj_doc]
            pub type #base_arc_trait_obj_ident<'cglue_a, CGlueF, CGlueC, #gen_use>
                = #base_ctx_trait_obj_ident<'cglue_a, CGlueF, #crate_path::arc::COptArc<CGlueC>, #gen_use>;

            #[doc = #base_arc_trait_obj_doc]
            pub type #base2_arc_trait_obj_ident<'cglue_a, CGlueF, CGlueC, CGlueD, #gen_use>
                = #base2_ctx_trait_obj_ident<'cglue_a, CGlueF, #crate_path::arc::COptArc<CGlueC>, #crate_path::arc::COptArc<CGlueD>, #gen_use>;

            #[doc = #base_mut_trait_obj_doc]
            pub type #base_mut_trait_obj_ident<'cglue_a, CGlueF, #gen_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a mut CGlueF, CGlueF, #trg_path::NoContext, #trg_path::NoContext, #gen_use>;

            #[doc = #base_ref_trait_obj_doc]
            pub type #base_ref_trait_obj_ident<'cglue_a, CGlueF, #gen_use>
                = #base_trait_obj_ident<'cglue_a, &'cglue_a CGlueF, CGlueF, #trg_path::NoContext, #trg_path::NoContext, #gen_use>;

            #[doc = #base_trait_obj_doc]
            pub type #base_trait_obj_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                = #trg_path::CGlueTraitObj::<'cglue_a, CGlueT, #vtbl_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>, #ret_tmp_ident<#gen_use>>;

            #[doc = #opaque_box_trait_obj_doc]
            pub type #opaque_box_trait_obj_ident<'cglue_a, #gen_use>
                = #base_box_trait_obj_ident<'cglue_a, #c_void, #gen_use>;

            #[doc = #opaque_ctx_trait_obj_doc]
            pub type #opaque_ctx_trait_obj_ident<'cglue_a, CGlueD, #gen_use>
                = #base2_ctx_trait_obj_ident<'cglue_a, #c_void, CGlueD, CGlueD, #gen_use>;

            #[doc = #opaque_no_ctx_trait_obj_doc]
            pub type #opaque_no_ctx_trait_obj_ident<'cglue_a, #gen_use>
                = #base_no_ctx_trait_obj_ident<'cglue_a, #c_void, #gen_use>;

            #[doc = #opaque_arc_trait_obj_doc]
            pub type #opaque_arc_trait_obj_ident<'cglue_a, #gen_use>
                = #base2_arc_trait_obj_ident<'cglue_a, #c_void, #c_void, #c_void, #gen_use>;

            #[doc = #opaque_mut_trait_obj_doc]
            pub type #opaque_mut_trait_obj_ident<'cglue_a, #gen_use>
                = #base_mut_trait_obj_ident<'cglue_a, #c_void, #gen_use>;

            #[doc = #opaque_ref_trait_obj_doc]
            pub type #opaque_ref_trait_obj_ident<'cglue_a, #gen_use>
                = #base_ref_trait_obj_ident<'cglue_a, #c_void, #gen_use>;

            #[doc = #opaque_trait_obj_doc]
            pub type #opaque_trait_obj_ident<'cglue_a, CGlueT, CGlueD, #gen_use>
                = #base_trait_obj_ident<'cglue_a, CGlueT, #c_void, CGlueD, CGlueD, #gen_use>;

            /* Internal wrapper functions. */

            #cfuncs

            /* Trait implementation. */

            impl<'cglue_a, #life_declare CGlueT #cglue_t_bounds_opaque, #cglue_f_in_impl CGlueD: #cd_bounds + #cd_opaque_bound, CGlueO: 'cglue_a + #trg_path::GetVtbl<#opaque_vtbl_ident<'cglue_a, CGlueT, CGlueD, #gen_use>> + #trg_path::#required_mutability<#ret_tmp_ident<#gen_use>, ObjType = #c_void, ContType = CGlueT, Context = CGlueD> #return_self_bound #supertrait_bounds, #gen_declare> #trait_impl_name<#life_use #gen_use> for CGlueO where #gen_where_bounds
            {
                #trait_type_defs
                #trait_impl_fns
            }

            #internal_trait_impl
        }
    }
}
