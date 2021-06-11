use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::{ParsedFunc, WrappedType};
use super::generics::{GenericType, ParsedGenerics};

use quote::*;
use syn::*;

pub fn process_item(
    ty: &TraitItemType,
    trait_type_defs: &mut TokenStream,
    types: &mut BTreeMap<Ident, WrappedType>,
    crate_path: &TokenStream,
) {
    let c_void = quote!(::core::ffi::c_void);

    let static_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("static"),
    };

    let cglue_c_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("cglue_c"),
    };

    let mut lifetime_bounds = ty.bounds.iter().filter_map(|b| match b {
        TypeParamBound::Lifetime(lt) => Some(lt),
        _ => None,
    });

    let lifetime_bound = lifetime_bounds.next();
    // TODO: warn user if multiple bounds exist, and an object is being created
    // let multiple_lifetime_bounds = lifetime_bounds.next().is_some();

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
                        return_conv: None,
                        lifetime_bound: None,
                        other_bounds: None,
                        impl_return_conv: None,
                        inject_ret_tmp: false,
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

                if x == "wrap_with_obj" {
                    new_ty.target =
                        format_ident!("CGlueBox{}", target.to_string()).to_token_stream();
                } else if x == "wrap_with_obj_ref" {
                    new_ty.target =
                        format_ident!("CGlueRef{}", target.to_string()).to_token_stream();
                } else if x == "wrap_with_obj_mut" {
                    new_ty.target =
                        format_ident!("CGlueMut{}", target.to_string()).to_token_stream();
                }

                // These variables model a `SomeGroup: From<CGlueF::#ty_ident>` bound.
                let mut from_new_ty_hrtb = None;
                let mut from_new_ty = new_ty.clone();
                let mut from_new_ty_ref = TokenStream::new();

                let lifetime = lifetime_bound.unwrap_or(&static_lifetime);

                // Insert the object lifetime at the start
                new_ty.push_lifetime_start(lifetime);

                let from_lifetime = if x == "wrap_with_group" || lifetime != &static_lifetime {
                    lifetime
                } else {
                    from_new_ty_hrtb = Some(quote!(for<'cglue_c>));
                    &cglue_c_lifetime
                };

                from_new_ty.push_lifetime_start(from_lifetime);

                let ty_ident = &ty.ident;

                if x == "wrap_with_group" {
                    new_ty.push_types_start(quote!(#crate_path::boxed::CBox<#c_void>, #c_void,));
                    from_new_ty.push_types_start(
                        quote!(#crate_path::boxed::CBox<CGlueF::#ty_ident>, CGlueF::#ty_ident,),
                    );
                } else if x == "wrap_with_group_ref" {
                    new_ty.push_types_start(quote!(&#lifetime #c_void, #c_void,));
                    from_new_ty.push_types_start(
                        quote!(&#from_lifetime CGlueF::#ty_ident, CGlueF::#ty_ident,),
                    );
                    from_new_ty_ref.extend(quote!(&#from_lifetime));
                } else if x == "wrap_with_group_mut" {
                    new_ty.push_types_start(quote!(&#lifetime mut #c_void, #c_void,));
                    from_new_ty.push_types_start(
                        quote!(&#from_lifetime mut CGlueF::#ty_ident, CGlueF::#ty_ident,),
                    );
                    from_new_ty_ref.extend(quote!(&#from_lifetime mut));
                }

                trait_type_defs.extend(quote!(type #ty_ident = #new_ty;));

                let type_bounds = if ["wrap_with_obj", "wrap_with_obj_ref", "wrap_with_obj_mut"]
                    .contains(&x)
                {
                    //trait_type_bounds.extend(quote!(CGlueF::#ty_ident: #lifetime, ));
                    None
                } else {
                    let type_bounds = quote!(#from_new_ty_hrtb #from_new_ty: From<#from_new_ty_ref CGlueF::#ty_ident>,);

                    //trait_type_bounds
                    //    .extend(quote!(CGlueF::#ty_ident: #lifetime, #type_bounds));

                    Some(type_bounds)
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
                    (
                        quote! {
                            // SAFETY:
                            // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                            // returning reference to the object.
                            unsafe {
                                ret_tmp.as_mut_ptr().write(std::mem::transmute(ret));
                            }
                        },
                        quote!(),
                    )
                };

                let (return_conv, inject_ret_tmp) = match x {
                    "wrap_with_obj" => (
                        parse2(quote!(|ret| trait_obj!(ret as #target)))
                            .expect("Internal closure parsing fail"),
                        false,
                    ),
                    "wrap_with_group" => (
                        parse2(quote!(|ret| group_obj!(ret as #target)))
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

                types.insert(
                    ty_ident.clone(),
                    WrappedType {
                        ty: new_ty,
                        return_conv: Some(return_conv),
                        impl_return_conv: None,
                        lifetime_bound: Some(lifetime.clone()),
                        other_bounds: type_bounds,
                        inject_ret_tmp,
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
            return_conv: None,
            lifetime_bound: None,
            other_bounds: Some(quote!(CGlueT: From<CGlueF>,)),
            impl_return_conv: Some(quote! {
                // SAFETY:
                //
                // The C wrapper type checks that the same object gets returned
                // as the vtables inside `self`.
                unsafe { self.cobj_build(ret) }
            }),
            inject_ret_tmp: false,
        },
    );

    let int_result = tr
        .attrs
        .iter()
        .any(|a| a.path.to_token_stream().to_string() == "int_result");

    // Parse all functions in the trait
    for item in &tr.items {
        match item {
            // We assume types are defined before methods here...
            TraitItem::Type(ty) => process_item(ty, &mut trait_type_defs, &mut types, crate_path),
            TraitItem::Method(m) => {
                let attrs = m
                    .attrs
                    .iter()
                    .map(|a| a.path.to_token_stream().to_string())
                    .collect::<Vec<_>>();

                if attrs.iter().any(|i| i == "skip_func") {
                    continue;
                }

                if !m.sig.generics.params.is_empty() {
                    if m.default.is_none() {
                        panic!("Generic function `{}` detected without a default implementation! This is not supported.", m.sig.ident);
                    }
                    continue;
                }

                let int_result = match int_result {
                    true => !attrs.iter().any(|i| i == "no_int_result"),
                    false => attrs.iter().any(|i| i == "int_result"),
                };

                funcs.extend(ParsedFunc::new(
                    m.sig.clone(),
                    trait_name.clone(),
                    &generics,
                    &types,
                    int_result,
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
    let vtbl_ident = format_ident!("CGlueVtbl{}", trait_name);
    let ret_tmp_ident = format_ident!("CGlueRetTmp{}", trait_name);
    let ret_tmp_ident_phantom = format_ident!("CGlueRetTmpPhantom{}", trait_name);
    let opaque_vtbl_ident = format_ident!("Opaque{}", vtbl_ident);
    let trait_obj_ident = format_ident!("CGlueBase{}", trait_name);
    let opaque_owned_trait_obj_ident = format_ident!("CGlueBox{}", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("CGlueMut{}", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("CGlueRef{}", trait_name);

    let (funcs, generics, trait_type_defs) = parse_trait(&tr, &crate_path, process_item);

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

    for func in funcs.iter() {
        let extra_bounds = func.cfunc_def(&mut cfuncs, &trg_path);
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

    let return_self_bound = if return_self {
        quote!(+ #trg_path::CGlueObjBuild<#ret_tmp_ident<#life_use #gen_use>>)
    } else {
        quote!()
    };

    let (cglue_t_bounds, cglue_t_bounds_opaque) = if need_own {
        (
            quote!(: ::core::ops::Deref<Target = CGlueF> + #trg_path::IntoInner<InnerTarget = CGlueF>),
            quote!(: ::core::ops::Deref<Target = #c_void> + #trg_path::IntoInner),
        )
    } else {
        (
            quote!(: ::core::ops::Deref<Target = CGlueF>),
            quote!(: ::core::ops::Deref<Target = #c_void>),
        )
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
    let trait_obj_doc = format!(" CGlue Trait Object type for trait {}.", trait_name);

    let opaque_owned_trait_obj_doc =
        format!(" Owned Opaque CGlue Trait Object for trait {}.", trait_name);
    let opaque_ref_trait_obj_doc = format!(
        " By-Ref Opaque CGlue Trait Object for trait {}.",
        trait_name
    );
    let opaque_mut_trait_obj_doc = format!(
        " By-Mut Opaque CGlue Trait Object for trait {}.",
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
            pub struct #ret_tmp_ident<#life_use #gen_use> {
                #ret_tmp_type_defs
                #phantom_data_definitions
            }

            impl<#life_use #gen_use> #ret_tmp_ident<#life_use #gen_use> {
                #ret_tmp_getter_defs
            }

            impl<#life_use #gen_use> Default for #ret_tmp_ident<#life_use #gen_use> {
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
            pub struct #ret_tmp_ident_phantom<#life_use #gen_use> {
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
            pub type #ret_tmp_ident<#life_use #gen_use> = ::core::marker::PhantomData<#ret_tmp_ident_phantom<#life_use #gen_use>>;
        }
    };

    // Glue it all together
    quote! {
        #tr

        #vis use #submod_name::{
            #vtbl_ident,
            #ret_tmp_ident,
            #opaque_vtbl_ident,
            #trait_obj_ident,
            #opaque_owned_trait_obj_ident,
            #opaque_mut_trait_obj_ident,
            #opaque_ref_trait_obj_ident,
        };

        mod #submod_name {
            use super::*;
            use super::#trait_impl_name;

            /* Primary vtable definition. */

            #[doc = #vtbl_doc]
            ///
            /// This virtual function table contains ABI-safe interface for the given trait.
            #[repr(C)]
            pub struct #vtbl_ident<#life_declare CGlueT, CGlueF, #gen_declare> where #gen_where_bounds {
                #vtbl_func_defintions
                #vtbl_phantom_def
            }

            impl<#life_declare CGlueT, CGlueF, #gen_declare> #vtbl_ident<#life_use CGlueT, CGlueF, #gen_use>
                where #gen_where_bounds
            {
                #vtbl_getter_defintions
            }

            #ret_tmp

            /* Default implementation. */

            /// Default vtable reference creation.
            impl<'cglue_a, #life_declare CGlueT #cglue_t_bounds, CGlueF: #trait_name<#life_use #gen_use>, #gen_declare> Default for &'cglue_a #vtbl_ident<#life_use CGlueT, CGlueF, #gen_use> where #gen_where_bounds #trait_type_bounds {
                /// Create a static vtable for the given type.
                fn default() -> Self {
                    &#vtbl_ident {
                        #vtbl_default_funcs
                        #vtbl_phantom_init
                    }
                }
            }

            /* Vtable trait implementations. */

            #[doc = #vtbl_opaque_doc]
            ///
            /// This virtual function table has type information destroyed, is used in CGlue objects
            /// and trait groups.
            pub type #opaque_vtbl_ident<#life_use CGlueT, #gen_use> = #vtbl_ident<#life_use CGlueT, #c_void, #gen_use>;

            unsafe impl<#life_declare CGlueT: #trg_path::Opaquable, CGlueF: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueBaseVtbl for #vtbl_ident<#life_use CGlueT, CGlueF, #gen_use> where #gen_where_bounds {
                type OpaqueVtbl = #opaque_vtbl_ident<#life_use CGlueT::OpaqueTarget, #gen_use>;
                type RetTmp = #ret_tmp_ident<#life_use #gen_use>;
            }

            impl<#life_declare CGlueT #cglue_t_bounds, CGlueF: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueVtbl<CGlueF> for #vtbl_ident<#life_use CGlueT, CGlueF, #gen_use> where #gen_where_bounds CGlueT: #trg_path::Opaquable {}

            #[doc = #trait_obj_doc]
            pub type #trait_obj_ident<'cglue_a, #life_use CGlueT, CGlueF, #gen_use> = #trg_path::CGlueTraitObj::<'cglue_a, CGlueT, #vtbl_ident<#life_use CGlueT, CGlueF, #gen_use>, #ret_tmp_ident<#life_use #gen_use>>;

            #[doc = #opaque_owned_trait_obj_doc]
            pub type #opaque_owned_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #crate_path::boxed::CBox<#c_void>, #c_void, #gen_use>;

            #[doc = #opaque_mut_trait_obj_doc]
            pub type #opaque_mut_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use &'cglue_a mut #c_void, #c_void, #gen_use>;

            #[doc = #opaque_ref_trait_obj_doc]
            pub type #opaque_ref_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use &'cglue_a #c_void, #c_void, #gen_use>;

            /* Internal wrapper functions. */

            #cfuncs

            /* Trait implementation. */

            /// Implement the traits for any CGlue object.
            impl<#life_declare CGlueT #cglue_t_bounds_opaque, CGlueO: #trg_path::GetVtbl<#opaque_vtbl_ident<#life_use CGlueT, #gen_use>> + #trg_path::#required_mutability<#ret_tmp_ident<#life_use #gen_use>, ObjType = #c_void, ContType = CGlueT> #return_self_bound #supertrait_bounds, #gen_declare> #trait_impl_name<#life_use #gen_use> for CGlueO where #gen_where_bounds {
                #trait_type_defs
                #trait_impl_fns
            }

            #internal_trait_impl
        }
    }
}