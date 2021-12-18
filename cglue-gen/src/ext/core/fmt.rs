use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

const FMT_TRAITS: &[(&str, &str)] = &[
    ("Display", ""),
    ("Debug", "?"),
    ("Octal", "o"),
    ("LowerHex", "x"),
    ("UpperHex", "X"),
    ("Pointer", "p"),
    ("Binary", "b"),
    ("LowerExp", "e"),
    ("UpperExp", "E"),
];

// fmt is a tricky beast, we need to employ custom wrapping to make it work across FFI-boundary.
fn get_custom_impl(fmt_str: &str, crate_path: &TokenStream) -> TokenStream {
    quote! {
        #[custom_impl(
            // Types within the C interface other than self and additional wrappers.
            {
                f_out: &mut WriteMut,
            },
            // Unwrapped return type
            Result<(), ::core::fmt::Error>,
            // Conversion in trait impl to C arguments (signature names are expected).
            {
                let f_out: WriteBaseMut<::core::fmt::Formatter> = From::from(f);
                let f_out = &mut #crate_path::trait_group::Opaquable::into_opaque(f_out);
            },
            // This is the body of C impl minus the automatic wrapping.
            {
                write!(f_out, #fmt_str, this)
            },
            // This part is processed in the trait impl after the call returns (impl_func_ret,
            // nothing extra needs to happen here).
            {
            },
        )]
    }
}

pub fn fmt_impls() -> TokenStream {
    let mut out = TokenStream::new();

    let crate_path = crate::util::crate_path();

    for (name, ty) in FMT_TRAITS {
        let wrap_impl = get_custom_impl(&format!("{{:{}}}", ty), &crate_path);
        let ident = format_ident!("{}", name);

        out.extend(quote! {
            pub trait #ident {
                #[int_result]
                #wrap_impl
                fn fmt(
                    &self,
                    f: &mut ::core::fmt::Formatter
                ) -> Result<(), ::core::fmt::Error>;
            }
        });
    }

    out
}

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));

    let fmt_impls = fmt_impls();

    out.push((
        cur_path,
        quote! {
            pub trait Write {
                #[int_result]
                fn write_str(&mut self, s: &str) -> Result<(), ::core::fmt::Error>;
            }

            #fmt_impls
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("fmt"));
    exports.insert(format_ident!("Debug"), cur_path.clone());
    exports.insert(format_ident!("Display"), cur_path);
}
