use proc_macro2::TokenStream;

use super::func::{ParsedFunc, ParsedGenerics};

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
        gen_where,
        ..
    } = &generics;

    let c_void = quote!(::core::ffi::c_void);

    // Additional identifiers
    let vtbl_ident = format_ident!("CGlueVtbl{}", trait_name);
    let opaque_vtbl_ident = format_ident!("Opaque{}", vtbl_ident);
    let trait_obj_ident = format_ident!("CGlueTraitObj{}", trait_name);
    let opaque_owned_trait_obj_ident = format_ident!("CGlueOpaqueTraitObj{}", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("CGlueMutOpaqueTraitObj{}", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("CGlueRefOpaqueTraitObj{}", trait_name);

    let mut funcs = vec![];

    let int_result = tr
        .attrs
        .iter()
        .any(|a| a.path.to_token_stream().to_string() == "int_result");

    // Parse all functions in the trait
    for item in &tr.items {
        if let TraitItem::Method(m) = item {
            let mut iter = m.attrs.iter().map(|a| a.path.to_token_stream().to_string());

            let int_result = match int_result {
                true => !iter.any(|i| i == "no_int_result"),
                false => iter.any(|i| i == "int_result"),
            };

            funcs.extend(ParsedFunc::new(
                m.sig.clone(),
                trait_name.clone(),
                &generics,
                int_result,
                &crate_path,
            ));
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

    // Implement the trait for a type that has AsRef<OpaqueCGlueVtblT>
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
        #vis struct #vtbl_ident<#life_use CGlueT, #gen_use> #gen_where {
            #vtbl_func_defintions
        }

        /* Default implementation. */

        /// Default vtable reference creation.
        impl<'cglue_a, #life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> Default for &'cglue_a #vtbl_ident<#life_use CGlueT, #gen_use> #gen_where {
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

        unsafe impl<#life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueBaseVtbl for #vtbl_ident<#life_use CGlueT, #gen_use> #gen_where {
            type OpaqueVtbl = #opaque_vtbl_ident<#life_use #gen_use>;
        }

        impl<#life_declare CGlueT: #trait_name<#life_use #gen_use>, #gen_declare> #trg_path::CGlueVtbl<CGlueT> for #vtbl_ident<#life_use CGlueT, #gen_use> #gen_where {}

        #[doc = #trait_obj_doc]
        pub type #trait_obj_ident<'cglue_a, #life_use CGlueT, B, #gen_use> = #trg_path::CGlueTraitObj::<'cglue_a, B, #vtbl_ident<#life_use CGlueT, #gen_use>>;

        #[doc = #opaque_owned_trait_obj_doc]
        pub type #opaque_owned_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #c_void, #crate_path::boxed::CBox<#c_void>, #gen_use>;

        #[doc = #opaque_mut_trait_obj_doc]
        pub type #opaque_mut_trait_obj_ident<'cglue_a, #life_use #gen_use> = #trait_obj_ident<'cglue_a, #life_use #c_void, &'cglue_a mut #c_void, #gen_use>;

        #opaque_ref_trait_obj

        /* Internal wrapper functions. */

        #cfuncs

        /* Trait implementation. */

        /// Implement the traits for any CGlue object.
        impl<#life_declare CGlueT: AsRef<#opaque_vtbl_ident<#life_use #gen_use>> + #trg_path::#required_mutability<#c_void>, #gen_declare> #trait_name<#life_use #gen_use> for CGlueT #gen_where {
            #trait_impl_fns
        }
    }
}
