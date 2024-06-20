use proc_macro2::TokenStream;

use std::collections::BTreeMap;

use super::func::WrappedType;
use super::generics::ParsedGenerics;

use quote::*;
use syn::*;

pub fn gen_forward(tr: ItemTrait, ext_path: Option<TokenStream>) -> TokenStream {
    let crate_path = crate::util::crate_path();

    let mut types = BTreeMap::new();

    types.insert(
        format_ident!("Self"),
        WrappedType {
            ty: parse2(quote!(Self)).unwrap(),
            ty_ret_tmp: None,
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

    let mut wrapped_types = TokenStream::new();

    let (funcs, generics, _, _) = super::traits::parse_trait(
        &tr,
        &crate_path,
        false,
        |(ty_ident, _, ty_where_clause, _), _, _, _, _, _, _| {
            if let Some(ident) = ty_ident {
                wrapped_types.extend(quote!(type #ident = CGlueT::#ident #ty_where_clause;));
            }
        },
    );

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

    let mut need_mut = false;

    for func in funcs {
        let nm = func.forward_wrapped_trait_impl(&mut impls);
        need_mut = nm || need_mut;
    }

    let mut required_mutability = TokenStream::new();

    required_mutability.extend(quote!(::core::ops::Deref<Target = CGlueT>));

    if need_mut {
        required_mutability.extend(quote!( + ::core::ops::DerefMut))
    }

    let tr_impl = if ext_path.is_some() {
        quote!()
    } else {
        quote!(#tr)
    };

    let needs_send = tr.supertraits.iter().any(|s| {
        if let TypeParamBound::Trait(tr) = s {
            tr.path.get_ident().map(|i| i == "Send") == Some(true)
        } else {
            false
        }
    });

    let send_bound = if needs_send { quote!(+ Send) } else { quote!() };
    quote! {
        #tr_impl

        impl<#life_declare CGlueO: #required_mutability #send_bound, CGlueT, #gen_declare> #ext_path #trait_name<#life_use #gen_use> for #crate_path::forward::Fwd<CGlueO> where CGlueT: #ext_path #trait_name<#life_use #gen_use>, #gen_where_bounds {
            #wrapped_types
            #impls
        }
    }
}
