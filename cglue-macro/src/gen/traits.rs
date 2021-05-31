use proc_macro2::TokenStream;

use super::func::ParsedFunc;

use quote::*;
use syn::*;

pub fn gen_trait(tr: &ItemTrait) -> TokenStream {
    // Path to trait group import.
    let crate_path = crate::util::crate_path();
    let trg_path: TokenStream = quote!(#crate_path::trait_group);

    // Need to preserve the same visibility as the trait itself.
    let vis = tr.vis.to_token_stream();

    let trait_name = &tr.ident;

    let c_void = quote!(::core::ffi::c_void);

    // Additional identifiers
    let vtbl_ident = format_ident!("CGlueVtbl{}", trait_name);
    let opaque_vtbl_ident = format_ident!("Opaque{}", vtbl_ident);
    let trait_obj_ident = format_ident!("CGlueTraitObj{}", trait_name);
    let opaque_owned_trait_obj_ident = format_ident!("CGlueOpaqueTraitObj{}", trait_name);
    let opaque_mut_trait_obj_ident = format_ident!("CGlueMutOpaqueTraitObj{}", trait_name);
    let opaque_ref_trait_obj_ident = format_ident!("CGlueRefOpaqueTraitObj{}", trait_name);

    let mut funcs = vec![];

    // Parse all functions in the trait
    for item in &tr.items {
        if let TraitItem::Method(m) = item {
            funcs.extend(ParsedFunc::new(
                m.sig.clone(),
                trait_name.clone(),
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

    // If the trait has funcs with mutable self, disallow &T objects.
    let required_mutability = match need_mut {
        true => quote!(CGlueObjMut),
        _ => quote!(CGlueObjRef),
    };

    // Formatted documentation strings
    let vtbl_doc = format!("CGlue vtable for trait {}.", trait_name);
    let vtbl_opaque_doc = format!("Opaque CGlue vtable for trait {}.", trait_name);
    let trait_obj_doc = format!("CGlue Trait Object type for trait {}.", trait_name);

    let opaque_owned_trait_obj_doc =
        format!("Owned Opaque CGlue Trait Object for trait {}.", trait_name);
    let opaque_ref_trait_obj_doc =
        format!("By-Ref Opaque CGlue Trait Object for trait {}.", trait_name);
    let opaque_mut_trait_obj_doc =
        format!("By-Mut Opaque CGlue Trait Object for trait {}.", trait_name);

    let opaque_ref_trait_obj = match need_mut {
        true => quote!(),
        false => quote! {
            #[doc = #opaque_ref_trait_obj_doc]
            pub type #opaque_ref_trait_obj_ident<'a> = #trait_obj_ident<'a, #c_void, &'a #c_void>;
        },
    };

    // Glue it all together
    quote! {
        /* Primary vtable definition. */

        #[doc = #vtbl_doc]
        ///
        /// This virtual function table contains ABI-safe interface for the given trait.
        #[repr(C)]
        #vis struct #vtbl_ident<T> {
            #vtbl_func_defintions
        }

        /* Default implementation. */

        /// Default vtable reference creation.
        impl<'a, T: #trait_name> Default for &'a #vtbl_ident<T> {
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

        #vis type #opaque_vtbl_ident = #vtbl_ident<#c_void>;

        unsafe impl<T: #trait_name> #trg_path::CGlueBaseVtbl for #vtbl_ident<T> {
            type OpaqueVtbl = #opaque_vtbl_ident;
        }

        impl<T: #trait_name> #trg_path::CGlueVtbl<T> for #vtbl_ident<T> {}

        #[doc = #trait_obj_doc]
        pub type #trait_obj_ident<'a, T, B> = #trg_path::CGlueTraitObj::<'a, B, #vtbl_ident<T>>;

        #[doc = #opaque_owned_trait_obj_doc]
        pub type #opaque_owned_trait_obj_ident<'a> = #trait_obj_ident<'a, #c_void, #crate_path::boxed::CBox<#c_void>>;

        #[doc = #opaque_mut_trait_obj_doc]
        pub type #opaque_mut_trait_obj_ident<'a> = #trait_obj_ident<'a, #c_void, &'a mut #c_void>;

        #opaque_ref_trait_obj

        /* Internal wrapper functions. */

        #cfuncs

        /* Trait implementation. */

        /// Implement the traits for any CGlue object.
        impl<T: AsRef<#opaque_vtbl_ident> + #trg_path::#required_mutability<#c_void>> #trait_name for T {
            #trait_impl_fns
        }
    }
}
