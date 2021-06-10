use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::WrappedType;
use super::generics::ParsedGenerics;

use quote::*;
use syn::*;

pub fn gen_wrap(tr: ItemTrait, ext_path: Option<TokenStream>) -> TokenStream {
    let crate_path = crate::util::crate_path();

    let mut types = BTreeMap::new();

    types.insert(
        format_ident!("Self"),
        WrappedType {
            ty: parse2(quote!(Self)).unwrap(),
            return_conv: None,
            lifetime_bound: None,
            other_bounds: None,
            impl_return_conv: None,
            inject_ret_tmp: false,
        },
    );

    let mut wrapped_types = TokenStream::new();

    let (funcs, generics, _) = super::traits::parse_trait(&tr, &crate_path, |ty, _, types, _| {
        let mut has_wrapped = false;
        let ident = &ty.ident;

        for attr in &ty.attrs {
            let s = attr.path.to_token_stream().to_string();

            if s.as_str() == "arc_wrap" {
                let new_ty =
                    parse2(quote!(#crate_path::arc::ArcWrapped<CGlueT::#ident, CGlueA>)).unwrap();
                wrapped_types.extend(quote!(type #ident = #new_ty;));

                types.insert(
                    ident.clone(),
                    WrappedType {
                        ty: new_ty,
                        return_conv: None,
                        lifetime_bound: None,
                        other_bounds: None,
                        impl_return_conv: None,
                        inject_ret_tmp: false,
                    },
                );

                has_wrapped = true;
            }
        }

        if !has_wrapped {
            wrapped_types.extend(quote!(type #ident = CGlueT::#ident;));
        }
    });

    let ParsedGenerics {
        life_declare,
        life_use,
        gen_declare,
        gen_use,
        gen_where_bounds,
        ..
    } = &generics;

    let trait_name = &tr.ident;

    let mut impls = TokenStream::new();

    for func in &funcs {
        func.arc_wrapped_trait_impl(&mut impls);
    }

    let tr_impl = if ext_path.is_some() {
        quote!()
    } else {
        quote!(#tr)
    };

    quote! {
        #tr_impl

        impl<#life_declare CGlueT, CGlueA: 'static, #gen_declare> #ext_path #trait_name<#life_use #gen_use> for #crate_path::arc::ArcWrapped<CGlueT, CGlueA> where CGlueT: #ext_path #trait_name<#life_use #gen_use>, #gen_where_bounds {
            #wrapped_types
            #impls
        }
    }
}
