use proc_macro2::TokenStream;

use super::func::ParsedFunc;

use quote::{quote, ToTokens};
use syn::*;

pub fn gen_trait(tr: &ItemTrait) -> TokenStream {
    // Need to preserve the same visibility as the trait itself.
    let vis = tr.vis.to_token_stream().to_string();

    let trname = tr.ident.to_string();

    // Path to trait group import.
    let trg_path = format!("{}::trait_group", crate::util::crate_path().to_string());

    let mut funcs = vec![];

    // Parse all functions in the trait
    for item in &tr.items {
        if let TraitItem::Method(m) = item {
            funcs.push(ParsedFunc::new(m.sig.clone(), trname.clone()));
        }
    }

    let vtbl_name = format!("CGlueVtbl{}", trname);

    // Define the vtable
    let mut vtbl = format!(
        r#"
        /// CGlue vtable for trait {}.
        ///
        /// This virtual function table contains ABI-safe interface for the given trait.
        #[repr(C)]
        {} struct {}<T> {{
        "#,
        trname, vis, vtbl_name
    );

    for func in &funcs {
        vtbl.push_str(&func.vtbl_def());
    }

    vtbl.push_str("}");

    let parsed_vtbl: TokenStream = vtbl.parse().unwrap();

    // Define the default implementation for the vtable reference
    let mut vtbl_default = format!(
        r#"
        impl<'a, T: {}> Default for &'a {}<T> {{
            /// Create a static vtable for the given type.
            fn default() -> Self {{
                &{} {{
        "#,
        trname, vtbl_name, vtbl_name
    );

    for func in &funcs {
        vtbl_default.push_str(&func.vtbl_default_def());
    }

    vtbl_default.push_str(
        r#"
                }
            }
        }
        "#,
    );

    let parsed_vtbl_default: TokenStream = vtbl_default.parse().unwrap();

    // Define wrapped functions for the vtable
    let mut cfuncs = String::new();

    for func in funcs.iter().filter_map(ParsedFunc::cfunc_def) {
        cfuncs.push_str(&func);
    }

    let parsed_cfuncs: TokenStream = cfuncs.parse().unwrap();

    // Define safe opaque conversion for the vtable
    let mut vtbl_opaque = format!(
        r#"
        /// Opaque CGlue vtable for trait {}.
        ///
        /// This virtual function table has type information destroyed, is used in CGlue objects
        /// and trait groups.
        "#,
        trname
    );

    vtbl_opaque.push_str(&format!(
        r#"
        {} type Opaque{} = {}<core::ffi::c_void>;

        unsafe impl<T: {}> {}::CGlueBaseVtbl for {}<T> {{
            type OpaqueVtbl = Opaque{};
        }}

        impl<T: {}> {}::CGlueVtbl<T> for {}<T> {{}}

        /// CGlue Trait Object type for trait {}.
        pub type CGlueTraitObj{}<'a, T> = {}::CGlueTraitObj::<'a, T, CGlueVtbl{}<T>>;

        /// Opaque CGlue Trait Object for trait {}.
        pub type CGlueOpaqueTraitObj{}<'a> = CGlueTraitObj{}<'a, ::core::ffi::c_void>;
        "#,
        vis,
        vtbl_name,
        vtbl_name,
        trname,
        trg_path,
        vtbl_name,
        vtbl_name,
        trname,
        trg_path,
        vtbl_name,
        trname,
        trname,
        trg_path,
        trname,
        trname,
        trname,
        trname
    ));

    let parsed_vtbl_opaque: TokenStream = vtbl_opaque.parse().unwrap();

    // Implement the trait for a type that has AsRef<OpaqueCGlueVtblT>
    let mut trait_impl = format!(
        "impl<T: AsRef<Opaque{}> + {}::CGlueObj<core::ffi::c_void>> {} for T {{",
        vtbl_name, trg_path, trname
    );

    for func in &funcs {
        trait_impl.push_str(&func.trait_impl());
    }

    trait_impl.push('}');

    let parsed_trait_impl: TokenStream = trait_impl.parse().unwrap();

    // Glue it all together
    let gen = quote! {
        #parsed_vtbl
        #parsed_vtbl_default
        #parsed_cfuncs
        #parsed_vtbl_opaque
        #parsed_trait_impl
    };

    gen.into()
}
