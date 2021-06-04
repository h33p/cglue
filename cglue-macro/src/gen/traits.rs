use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::{ParsedFunc, WrappedType};
use super::generics::{GenericType, ParsedGenerics};

use quote::*;
use syn::*;

pub fn gen_trait(tr: &ItemTrait) -> TokenStream {
    // Path to trait group import.
    let crate_path = crate::util::crate_path();
    let trg_path: TokenStream = quote!(#crate_path::trait_group);

    // Need to preserve the same visibility as the trait itself.
    let vis = tr.vis.to_token_stream();

    let trait_name = &tr.ident;

    let generics = ParsedGenerics::from(&tr.generics);

    let ParsedGenerics {
        life_declare,
        life_use,
        gen_declare,
        gen_use,
        gen_where_bounds,
        ..
    } = &generics;

    let c_void = quote!(::core::ffi::c_void);

    // Additional identifiers
    let vtbl_ident = format_ident!("CGlueVtbl{}", trait_name);
    let ret_tmp_ident = format_ident!("CGlueRetTmp{}", trait_name);
    let opaque_vtbl_ident = format_ident!("Opaque{}", vtbl_ident);
    let trait_obj_ident = format_ident!("CGlueBase{}", trait_name);
    let opaque_owned_trait_obj_ident = format_ident!("CGlueBox{}", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("CGlueMut{}", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("CGlueRef{}", trait_name);

    let mut funcs = vec![];
    let mut types = BTreeMap::new();
    let mut trait_type_defs = TokenStream::new();
    let mut trait_type_bounds = TokenStream::new();

    let int_result = tr
        .attrs
        .iter()
        .any(|a| a.path.to_token_stream().to_string() == "int_result");

    let static_lifetime = Lifetime {
        apostrophe: proc_macro2::Span::call_site(),
        ident: format_ident!("static"),
    };

    // Parse all functions in the trait
    for item in &tr.items {
        match item {
            // We assume types are defined before methods here...
            TraitItem::Type(ty) => {
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
                                    inject_ret_tmp: false,
                                },
                            );
                        }
                        "return_wrap" => {
                            let closure = attr.parse_args::<ExprClosure>().expect(
                                "A valid closure must be supplied accepting the wrapped type!",
                            );

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
                                new_ty.target = format_ident!("CGlueBox{}", target.to_string())
                                    .to_token_stream();
                            } else if x == "wrap_with_obj_ref" {
                                new_ty.target = format_ident!("CGlueRef{}", target.to_string())
                                    .to_token_stream();
                            } else if x == "wrap_with_obj_mut" {
                                new_ty.target = format_ident!("CGlueMut{}", target.to_string())
                                    .to_token_stream();
                            }

                            let lifetime = lifetime_bound.unwrap_or(&static_lifetime);

                            // Insert the object lifetime at the start
                            new_ty.push_lifetime_start(lifetime);

                            if x == "wrap_with_group" {
                                new_ty.push_types_start(
                                    quote!(#crate_path::boxed::CBox<#c_void>, #c_void,),
                                );
                            } else if x == "wrap_with_group_ref" {
                                new_ty.push_types_start(quote!(&#lifetime #c_void, #c_void,));
                            } else if x == "wrap_with_group_mut" {
                                new_ty.push_types_start(quote!(&#lifetime mut #c_void, #c_void,));
                            }

                            let ty_ident = &ty.ident;

                            trait_type_defs.extend(quote!(type #ty_ident = #new_ty;));

                            let type_bounds = if [
                                "wrap_with_obj",
                                "wrap_with_obj_ref",
                                "wrap_with_obj_mut",
                            ]
                            .contains(&x)
                            {
                                trait_type_bounds.extend(quote!(CGlueT::#ty_ident: #lifetime, ));
                                None
                            } else {
                                let filler_trait =
                                    format_ident!("{}VtableFiller", new_ty.target.to_string());

                                let path = &new_ty.path;

                                let type_bounds = quote!(CGlueT::#ty_ident: #path #filler_trait<#lifetime, #life_use #gen_use>,);

                                trait_type_bounds
                                    .extend(quote!(CGlueT::#ty_ident: #lifetime, #type_bounds));

                                Some(type_bounds)
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
                                    parse2(quote!(|ret: &_| {
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
                                    parse2(quote!(|ret: &_| {
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
                                    parse2(quote!(|ret: &mut _| {
                                        let ret = trait_obj!(ret as #target);
                                        // SAFETY:
                                        // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                                        // returning reference to the object.
                                        unsafe {
                                            ret_tmp.as_mut_ptr().write(std::mem::transmute(ret))
                                        };
                                        unsafe { &mut *ret_tmp.as_mut_ptr() }
                                    }))
                                    .expect("Internal closure parsing fail"),
                                    true,
                                ),
                                "wrap_with_group_mut" => (
                                    parse2(quote!(|ret: &mut _| {
                                        let ret = group_obj!(ret as #target);
                                        // SAFETY:
                                        // We cast anon lifetime to static lifetime. It is rather okay, because we are only
                                        // returning reference to the object.
                                        unsafe {
                                            ret_tmp.as_mut_ptr().write(std::mem::transmute(ret))
                                        };
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
            TraitItem::Method(m) => {
                let mut iter = m.attrs.iter().map(|a| a.path.to_token_stream().to_string());

                let int_result = match int_result {
                    true => !iter.any(|i| i == "no_int_result"),
                    false => iter.any(|i| i == "int_result"),
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

    // Function definitions in the vtable
    let mut vtbl_func_defintions = TokenStream::new();

    for func in &funcs {
        func.vtbl_def(&mut vtbl_func_defintions);
    }

    // Default functions for vtable reference
    let mut vtbl_default_funcs = TokenStream::new();

    for func in &funcs {
        func.vtbl_default_def(&mut vtbl_default_funcs);
    }

    // Define wrapped functions for the vtable
    let mut cfuncs = TokenStream::new();

    for func in funcs.iter() {
        func.cfunc_def(&mut cfuncs);
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

    // Implement the trait for a type that has CGlueObj<OpaqueCGlueVtblT, RetTmp>
    let mut trait_impl_fns = TokenStream::new();

    let mut need_mut = false;

    for func in &funcs {
        need_mut = func.trait_impl(&mut trait_impl_fns) || need_mut;
    }

    // If the trait has funcs with mutable self, disallow &CGlueT objects.
    let required_mutability = match need_mut {
        true => quote!(CGlueObjMut),
        _ => quote!(CGlueObjRef),
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

    let opaque_ref_trait_obj = match need_mut {
        true => quote!(),
        false => quote! {
            #[doc = #opaque_ref_trait_obj_doc]
            pub type #opaque_ref_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #c_void, &'cglue_a #c_void, #gen_use>;
        },
    };

    // Glue it all together
    quote! {
        /* Primary vtable definition. */

        #[doc = #vtbl_doc]
        ///
        /// This virtual function table contains ABI-safe interface for the given trait.
        #[repr(C)]
        #vis struct #vtbl_ident<#life_declare CGlueT, #gen_declare> where #gen_where_bounds {
            #vtbl_func_defintions
        }

        #[repr(C)]
        #vis struct #ret_tmp_ident {
            #ret_tmp_type_defs
        }

        impl #ret_tmp_ident {
            #ret_tmp_getter_defs
        }

        impl Default for #ret_tmp_ident {
            fn default() -> Self {
                Self {
                    #ret_tmp_default_defs
                }
            }
        }

        /* Default implementation. */

        /// Default vtable reference creation.
        impl<'cglue_a, #life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> Default for &'cglue_a #vtbl_ident<#life_use CGlueT, #gen_use> where #gen_where_bounds #trait_type_bounds {
            /// Create a static vtable for the given type.
            fn default() -> Self {
                &#vtbl_ident {
                    #vtbl_default_funcs
                }
            }
        }

        /* Vtable trait implementations. */

        #[doc = #vtbl_opaque_doc]
        ///
        /// This virtual function table has type information destroyed, is used in CGlue objects
        /// and trait groups.
        #vis type #opaque_vtbl_ident<#life_use #gen_use> = #vtbl_ident<#life_use #c_void, #gen_use>;

        unsafe impl<#life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueBaseVtbl for #vtbl_ident<#life_use CGlueT, #gen_use> where #gen_where_bounds {
            type OpaqueVtbl = #opaque_vtbl_ident<#life_use #gen_use>;
            type RetTmp = #ret_tmp_ident;
        }

        impl<#life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueVtbl<CGlueT> for #vtbl_ident<#life_use CGlueT, #gen_use> where #gen_where_bounds {}

        #[doc = #trait_obj_doc]
        pub type #trait_obj_ident<'cglue_a, #life_use CGlueT, B, #gen_use> = #trg_path::CGlueTraitObj::<'cglue_a, B, #vtbl_ident<#life_use CGlueT, #gen_use>, #ret_tmp_ident>;

        #[doc = #opaque_owned_trait_obj_doc]
        pub type #opaque_owned_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #c_void, #crate_path::boxed::CBox<#c_void>, #gen_use>;

        #[doc = #opaque_mut_trait_obj_doc]
        pub type #opaque_mut_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #c_void, &'cglue_a mut #c_void, #gen_use>;

        #opaque_ref_trait_obj

        /* Internal wrapper functions. */

        #cfuncs

        /* Trait implementation. */

        /// Implement the traits for any CGlue object.
        impl<#life_declare CGlueT: #trg_path::CGlueObj<#opaque_vtbl_ident<#life_use #gen_use>, #ret_tmp_ident> + #trg_path::#required_mutability<#c_void, #ret_tmp_ident>, #gen_declare> #trait_name<#life_use #gen_use> for CGlueT where #gen_where_bounds {
            #trait_type_defs
            #trait_impl_fns
        }
    }
}
