use super::generics::{GenericType, ParsedGenerics};
use super::util::parse_brace_content;
use crate::util::{merge_lifetime_declarations, remap_lifetime_defs, remap_type_lifetimes};
use proc_macro2::TokenStream;
use quote::*;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use syn::{parse::*, punctuated::Punctuated, token::Comma, Type, *};

const FN_PREFIX: &str = "cglue_wrapped_";

pub struct WrappedType {
    pub ty: GenericType,
    pub ty_ret_tmp: Option<GenericType>,
    pub ty_static: Option<GenericType>,
    pub lifetime_bound: Option<Lifetime>,
    pub lifetime_type_bound: Option<Lifetime>,
    pub other_bounds: Option<TokenStream>,
    pub other_bounds_simple: Option<TokenStream>,
    pub return_conv: Option<ExprClosure>,
    pub impl_return_conv: Option<TokenStream>,
    pub inject_ret_tmp: bool,
    pub unbounded_hrtb: bool,
}

#[derive(Eq, PartialEq, Clone)]
pub struct AssocType {
    pub ident: Ident,
    pub generics: Generics,
}

impl AssocType {
    /// Remap the associated type for use within HRTB bounds.
    ///
    /// Currently the only supported configuration is a single generic lifetime.
    ///
    /// # Panics
    ///
    /// If generic types are not supported for remapping.
    pub fn remap_for_hrtb(&self) -> Self {
        let mut params = self.generics.params.iter();
        match (params.next(), params.next()) {
            (Some(GenericParam::Lifetime(_)), None) => Self {
                ident: self.ident.clone(),
                generics: syn::parse2(quote!(<'cglue_b>)).unwrap(),
            },
            (None, _) => self.clone(),
            _ => panic!("Unsupported generic parameter configuration!"),
        }
    }
}

impl ToTokens for AssocType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens);
        self.generics.to_tokens(tokens);
    }
}

impl PartialOrd for AssocType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AssocType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ident.cmp(&other.ident)
    }
}

impl From<Ident> for AssocType {
    fn from(ident: Ident) -> Self {
        Self {
            ident,
            generics: Default::default(),
        }
    }
}

impl AssocType {
    pub fn new(ident: Ident, generics: Generics) -> Self {
        Self { ident, generics }
    }
}

pub struct CustomFuncImpl {
    pub tys: Punctuated<FnArg, Comma>,
    pub c_ret_ty: ReturnType,
    pub pre_call_impl: TokenStream,
    pub c_inner_body: Option<TokenStream>,
    pub impl_func_ret: Option<TokenStream>,
}

#[derive(Default)]
struct CustomFuncConv {
    pub pre_call_impl: TokenStream,
    pub c_inner_body: Option<TokenStream>,
    pub impl_func_ret: Option<TokenStream>,
}

impl Parse for CustomFuncImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let content = parse_brace_content(input)?;
        let tys = Punctuated::parse_terminated(&content)?;

        input.parse::<Token![,]>()?;

        let c_ret_ty = ReturnType::Type(Default::default(), input.parse::<Type>()?.into());

        input.parse::<Token![,]>()?;

        let pre_call_impl: TokenStream = parse_brace_content(input)?.parse()?;
        input.parse::<Token![,]>()?;

        let c_inner_body: TokenStream = parse_brace_content(input)?.parse()?;
        let c_inner_body = if c_inner_body.is_empty() {
            None
        } else {
            Some(quote!( { #c_inner_body } ))
        };
        input.parse::<Token![,]>()?;

        let impl_func_ret: TokenStream = parse_brace_content(input)?.parse()?;
        let impl_func_ret = if impl_func_ret.is_empty() {
            None
        } else {
            Some(quote!( { #impl_func_ret } ))
        };

        input.parse::<Token![,]>().ok();

        Ok(Self {
            tys,
            c_ret_ty,
            pre_call_impl,
            c_inner_body,
            impl_func_ret,
        })
    }
}

/// TraitArgConv stores implementations for Unstable-C-Unstable ABI transitions.
struct TraitArgConv {
    /// Called in trait impl to define arguments. Useful when need to destruct a tuple/struct.
    to_c_args: TokenStream,
    /// Arguments inside the call to the C vtable function.
    call_c_args: TokenStream,
    /// C function signature.
    c_args: TokenStream,
    /// C function signature, where 'cglue_a lifetimes are replaced with 'cglue_b.
    c_cast_args: TokenStream,
    /// Arguments inside the call to the trait function.
    to_trait_arg: TokenStream,
}

fn ret_wrap_type<'a>(
    ty: &mut Type,
    targets: &'a BTreeMap<Option<AssocType>, WrappedType>,
) -> Option<(Type, Option<AssocType>, &'a WrappedType)> {
    // None means handle only the C side - the function will not be called on Rust side.
    // This is useful for providing functionality for C users that can be done faster in Rust.
    // TODO: perhaps switch targets to an enum to indicate C side or not.
    if let Some(wrapped) = targets.get(&None) {
        let WrappedType { ty: new_ty, .. } = wrapped;

        let ret = std::mem::replace(
            ty,
            syn::parse2(new_ty.to_token_stream()).expect("Failed to parse wrap_type"),
        );

        Some((ret, None, wrapped))
    } else {
        do_wrap_type(ty, targets)
    }
}

fn do_wrap_type<'a>(
    ty: &mut Type,
    targets: &'a BTreeMap<Option<AssocType>, WrappedType>,
) -> Option<(Type, Option<AssocType>, &'a WrappedType)> {
    match ty {
        Type::Reference(r) => do_wrap_type(&mut r.elem, targets),
        Type::Slice(s) => do_wrap_type(&mut s.elem, targets),
        Type::Path(p) => {
            let mut iter = p.path.segments.iter();
            match (&p.qself, p.path.leading_colon, iter.next(), iter.next()) {
                (None, None, Some(p1), Some(p2)) => {
                    if p1.ident == "Self" {
                        if let Some(wrapped) = targets.get(&Some(p2.ident.clone().into())) {
                            let WrappedType { ty: new_ty, .. } = wrapped;

                            std::mem::drop(iter);

                            let ident = p2.ident.clone();
                            let generics: Generics = syn::parse2(p2.arguments.to_token_stream())
                                .expect("Failed to parse generics");

                            let ret = std::mem::replace(
                                ty,
                                syn::parse2(new_ty.to_token_stream())
                                    .expect("Failed to parse wrap_type"),
                            );

                            return Some((ret, Some(AssocType::new(ident, generics)), wrapped));
                        }
                    }
                }
                (None, None, Some(p1), None) => {
                    if p1.ident == "Self" {
                        let self_return_wrap = targets
                            .get(&Some(p1.ident.clone().into()))
                            .expect("No self-wrap rule specified");
                        let WrappedType { ty: new_ty, .. } = self_return_wrap;

                        std::mem::drop(iter);

                        let ident = p1.ident.clone();
                        // Self has no type parameters, right?

                        let ret = std::mem::replace(
                            ty,
                            syn::parse2(new_ty.to_token_stream())
                                .expect("Failed to parse self-type wrap"),
                        );

                        return Some((ret, Some(ident.into()), self_return_wrap));
                    }
                }
                _ => {}
            }

            std::mem::drop(iter);

            for seg in p.path.segments.iter_mut() {
                if let PathArguments::AngleBracketed(brac) = &mut seg.arguments {
                    for arg in brac.args.iter_mut() {
                        if let GenericArgument::Type(ty) = arg {
                            let ret = do_wrap_type(ty, targets);
                            if ret.is_some() {
                                return ret;
                            }
                        }
                    }
                }
            }

            None
        }
        Type::Ptr(ptr) => do_wrap_type(&mut ptr.elem, targets),
        Type::Tuple(tup) => tup.elems.iter_mut().find_map(|e| do_wrap_type(e, targets)),
        // TODO: Other types
        _ => None,
    }
}

impl TraitArgConv {
    fn new(
        arg: &FnArg,
        targets: &BTreeMap<Option<AssocType>, WrappedType>,
        crate_path: &TokenStream,
        inject_lifetime: Option<&Lifetime>,
        inject_lifetime_cast: Option<&Lifetime>,
        lifetime_map: &BTreeMap<Lifetime, Lifetime>,
    ) -> Self {
        let (to_c_args, call_c_args, c_args, c_cast_args, to_trait_arg) = match arg {
            FnArg::Receiver(r) => {
                let lifetime = inject_lifetime.or_else(|| r.lifetime());
                let lifetime_cast = inject_lifetime_cast.or_else(|| r.lifetime());

                if r.reference.is_none() {
                    (
                        quote! {
                            let cont = self.into_ccont();
                            // Guard against failure cases where context drops the library.
                            // Only happens where self gets consumed.
                            // TODO: make a breaking change in cobj_owned so this is not needed
                            // separately
                            // TODO 2: figure out how to test this.
                            let __ctx = #crate_path::trait_group::CGlueObjBase::cobj_base_ref(&cont).1.clone();
                        },
                        quote!(cont,),
                        quote!(cont: CGlueC,),
                        quote!(cont: CGlueC,),
                        quote!(),
                    )
                } else if r.mutability.is_some() {
                    (
                        quote!(let cont = self.ccont_mut();),
                        quote!(cont,),
                        quote!(cont: &#lifetime mut CGlueC,),
                        quote!(cont: &#lifetime_cast mut CGlueC,),
                        quote!(),
                    )
                } else {
                    (
                        quote!(let cont = self.ccont_ref();),
                        quote!(cont,),
                        quote!(cont: &#lifetime CGlueC,),
                        quote!(cont: &#lifetime_cast CGlueC,),
                        quote!(),
                    )
                }
            }
            t if recv_pin(t) => {
                let lifetime = inject_lifetime.or_else(|| recv_lifetime(t));
                let lifetime_cast = inject_lifetime_cast.or_else(|| recv_lifetime(t));

                if recv_mutable(t) {
                    (
                        quote!(let cont = self.ccont_pin_mut();),
                        quote!(cont,),
                        quote!(cont: ::core::pin::Pin<&#lifetime mut CGlueC>,),
                        quote!(cont: ::core::pin::Pin<&#lifetime_cast mut CGlueC>,),
                        quote!(),
                    )
                } else {
                    (
                        quote!(let cont = self.ccont_pin_ref();),
                        quote!(cont,),
                        quote!(cont: ::core::pin::Pin<&#lifetime CGlueC>,),
                        quote!(cont: ::core::pin::Pin<&#lifetime_cast CGlueC>,),
                        quote!(),
                    )
                }
            }
            FnArg::Typed(t) => {
                let mut t = t.clone();
                let _old = do_wrap_type(&mut t.ty, targets);

                let name = &*t.pat;

                // TODO: handle more edge cases here
                let name = match name {
                    Pat::Ident(id) => id.ident.to_token_stream(),
                    x => quote!(#x),
                };

                let ty = &*t.ty;

                let mut ret = None;

                // Map all lifetimes
                let mut ty = ty.clone();
                remap_type_lifetimes(&mut ty, lifetime_map);

                // TODO: deal with nested conversion
                //if let (Some(old), Type::Path(p)) = {
                //}

                match &ty {
                    Type::Reference(r) => {
                        let is_mut = r.mutability.is_some();

                        let lt = r
                            .lifetime
                            .as_ref()
                            .map(|lt| lifetime_map.get(lt).unwrap_or(lt))
                            .map(|v| quote!(#v,));

                        let new_ty = match &*r.elem {
                            Type::Slice(s) => {
                                let ty = &*s.elem;
                                Some(if is_mut {
                                    quote!(#crate_path::slice::CSliceMut<#lt #ty>)
                                } else {
                                    quote!(#crate_path::slice::CSliceRef<#lt #ty>)
                                })
                                .map(|v| (v, false))
                            }
                            Type::Path(p) => if let Some("str") =
                                p.path.get_ident().map(|i| i.to_string()).as_deref()
                            {
                                Some(if is_mut {
                                    quote!(#crate_path::slice::CSliceMut<#lt u8>)
                                } else {
                                    quote!(#crate_path::slice::CSliceRef<#lt u8>)
                                })
                            } else {
                                None
                            }
                            .map(|v| (v, true)),
                            _ => None,
                        };

                        if let Some((slty, into_str)) = new_ty {
                            ret = Some((
                                quote!(),
                                quote!(#name.into(),),
                                quote!(#name: #slty,),
                                quote!(#name: #slty,),
                                if into_str {
                                    quote!(unsafe { #name.into_str() },)
                                } else {
                                    quote!(#name.into(),)
                                },
                            ))
                        }
                    }
                    // TODO: Warn if Box is being used.
                    Type::Path(p) => {
                        // Here we check for any Option types, and wrap them to COption if they can
                        // not be NPOd.
                        let last = p.path.segments.last();
                        if let Some((PathArguments::AngleBracketed(args), last)) =
                            last.map(|l| (&l.arguments, l))
                        {
                            match last.ident.to_string().as_str() {
                                "Option" => {
                                    if let Some(GenericArgument::Type(a)) = args.args.first() {
                                        if !crate::util::is_null_pointer_optimizable(a, &[]) {
                                            ret = Some((
                                                quote!(let #name = #name.into();),
                                                quote!(#name,),
                                                quote!(#name: #crate_path::option::COption<#a>,),
                                                quote!(#name: #crate_path::option::COption<#a>,),
                                                quote!(#name.into(),),
                                            ));
                                        }
                                    }
                                }
                                "Result" => {
                                    let mut args = args.args.iter();

                                    match (args.next(), args.next(), args.next(), false) {
                                        (Some(GenericArgument::Type(_)), _, None, true) => {
                                            // TODO: Wrap Result<T> alias to use int values if it is marked
                                            // to implement IntResult
                                        }
                                        (
                                            Some(GenericArgument::Type(a)),
                                            Some(GenericArgument::Type(b)),
                                            None,
                                            _,
                                        ) => {
                                            ret = Some((
                                                quote!(let #name = #name.into();),
                                                quote!(#name,),
                                                quote!(#name: #crate_path::result::CResult<#a, #b>,),
                                                quote!(#name: #crate_path::result::CResult<#a, #b>,),
                                                quote!(#name.into(),),
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Type::ImplTrait(t) => {
                        // Convert `impl Into<T>` to `T`.
                        if t.bounds.len() == 1 {
                            if let TypeParamBound::Trait(t) = t.bounds.first().unwrap() {
                                if t.path.segments.len() == 1 {
                                    let seg = t.path.segments.first().unwrap();
                                    if seg.ident == format_ident!("Into") {
                                        if let PathArguments::AngleBracketed(a) = &seg.arguments {
                                            if a.args.len() == 1 {
                                                let ty = a.args.first().unwrap();

                                                ret = Some((
                                                    quote!(let #name = #name.into();),
                                                    quote!(#name,),
                                                    quote!(#name: #ty,),
                                                    quote!(#name: #ty,),
                                                    quote!(#name,),
                                                ))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }

                ret.unwrap_or_else(|| {
                    (
                        quote!(let #name = #name;),
                        quote!(#name,),
                        quote!(#name: #ty,),
                        quote!(#name: #ty,),
                        quote!(#name,),
                    )
                })
            }
        };

        Self {
            to_c_args,
            call_c_args,
            c_args,
            c_cast_args,
            to_trait_arg,
        }
    }
}

pub struct ParsedFunc {
    name: Ident,
    trait_name: Ident,
    safe: bool,
    abi: FuncAbi,
    receiver: FnArg,
    orig_args: Vec<FnArg>,
    args: Vec<TraitArgConv>,
    out: ParsedReturnType,
    generics: ParsedGenerics,
    sig_generics: ParsedGenerics,
    custom_conv: CustomFuncConv,
    only_c_side: bool,
}

fn extract_pin(t: &Type) -> Option<&Type> {
    if let Type::Path(v) = t {
        if let Some(seg) = v.path.segments.last() {
            if seg.ident != "Pin" {
                return None;
            }
            if let PathArguments::AngleBracketed(a) = &seg.arguments {
                if a.args.len() == 1 {
                    let a = a.args.first()?;
                    if let GenericArgument::Type(t) = a {
                        return Some(t);
                    }
                }
            }
        }
    }
    None
}

fn recv_pin(recv: &FnArg) -> bool {
    match recv {
        FnArg::Receiver(_) => false,
        FnArg::Typed(t) => matches!(extract_pin(&t.ty), Some(Type::Reference(_))),
    }
}

fn recv_lifetime(recv: &FnArg) -> Option<&Lifetime> {
    match recv {
        FnArg::Receiver(r) => r.lifetime(),
        FnArg::Typed(t) => {
            if let Some(Type::Reference(r)) = extract_pin(&t.ty) {
                r.lifetime.as_ref()
            } else {
                None
            }
        }
    }
}

fn recv_reference(recv: &FnArg) -> bool {
    match recv {
        FnArg::Receiver(r) => r.reference.is_some(),
        FnArg::Typed(t) => matches!(extract_pin(&t.ty), Some(Type::Reference(_))),
    }
}

fn recv_mutable(recv: &FnArg) -> bool {
    match recv {
        FnArg::Receiver(r) => r.mutability.is_some(),
        FnArg::Typed(t) => matches!(
            extract_pin(&t.ty),
            Some(Type::Reference(TypeReference {
                mutability: Some(_),
                ..
            }))
        ),
    }
}

impl ParsedFunc {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sig: Signature,
        trait_name: Ident,
        generics: &ParsedGenerics,
        wrap_types: &BTreeMap<Option<AssocType>, WrappedType>,
        res_override: Option<&Ident>,
        int_result: bool,
        crate_path: &TokenStream,
        only_c_side: bool,
        custom_impl: Option<CustomFuncImpl>,
    ) -> Option<Self> {
        let name = sig.ident;
        let safe = sig.unsafety.is_none();
        let abi = From::from(sig.abi);
        let mut args: Vec<TraitArgConv> = vec![];
        let mut orig_args = vec![];

        let mut receiver = None;

        let unsafety = if safe { quote!(unsafe) } else { quote!() };

        for input in sig.inputs.iter() {
            match &input {
                FnArg::Receiver(_) => {
                    receiver = Some(input.clone());
                }
                FnArg::Typed(v) => {
                    if let Pat::Ident(ref i) = *v.pat {
                        if i.ident == "self" {
                            receiver = Some(input.clone());
                        }
                    }
                }
            }
        }

        let receiver = receiver?;

        let out = ParsedReturnType::new(
            (sig.output, custom_impl.as_ref().map(|i| &i.c_ret_ty)),
            wrap_types,
            res_override,
            int_result,
            &unsafety,
            (&name, &receiver),
            (crate_path, &trait_name, generics),
        );

        // If a custom impl is provided, use its arguments
        let custom_conv = if let Some(CustomFuncImpl {
            tys,
            pre_call_impl,
            c_inner_body,
            impl_func_ret,
            ..
        }) = custom_impl
        {
            orig_args.extend(sig.inputs.into_iter());
            // But first, we need to process the receiver (self) type, as it is implicit.
            for arg in orig_args
                .iter()
                .filter(|a| match a {
                    FnArg::Receiver(_) => true,
                    FnArg::Typed(v) => {
                        if let Pat::Ident(ref i) = *v.pat {
                            if i.ident == "self" {
                                return true;
                            }
                        }
                        false
                    }
                })
                .take(1)
                .chain(tys.iter())
            {
                args.push(TraitArgConv::new(
                    arg,
                    wrap_types,
                    crate_path,
                    out.lifetime.as_ref(),
                    out.lifetime_cast.as_ref(),
                    &out.lifetime_map,
                ));
            }

            CustomFuncConv {
                pre_call_impl,
                c_inner_body,
                impl_func_ret,
            }
        } else {
            for input in sig.inputs.into_iter() {
                let func = TraitArgConv::new(
                    &input,
                    wrap_types,
                    crate_path,
                    out.lifetime.as_ref(),
                    out.lifetime_cast.as_ref(),
                    &out.lifetime_map,
                );

                args.push(func);
                orig_args.push(input);
            }

            Default::default()
        };

        let generics = generics.clone();

        let sig_generics = ParsedGenerics::from(&sig.generics);

        Some(Self {
            name,
            trait_name,
            safe,
            abi,
            receiver,
            orig_args,
            args,
            out,
            generics,
            sig_generics,
            only_c_side,
            custom_conv,
        })
    }

    pub fn ret_tmp_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        // If injected_ret_tmp exists, try using the static one, but skip it if it doesn't exist.
        if let (Some(_), Some(ty)) = (
            &self.out.injected_ret_tmp,
            self.out
                .injected_ret_tmp_static
                .as_ref()
                .or(self.out.injected_ret_tmp.as_ref()),
        ) {
            let gen = if recv_mutable(&self.receiver) {
                quote!(#name: ::core::mem::MaybeUninit<#ty>,)
            } else {
                quote!(#name: ::core::cell::Cell<::core::mem::MaybeUninit<#ty>>,)
            };
            stream.extend(gen);
        }
    }

    pub fn ret_default_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        if self.out.injected_ret_tmp.is_some() {
            let gen = if recv_mutable(&self.receiver) {
                quote!(#name: ::core::mem::MaybeUninit::uninit(),)
            } else {
                quote!(#name: ::core::cell::Cell::new(::core::mem::MaybeUninit::uninit()),)
            };
            stream.extend(gen);
        }
    }

    pub fn ret_getter_def(&self, stream: &mut TokenStream) {
        let name = &self.name;

        if let Some(ty) = &self.out.injected_ret_tmp {
            let gen = match (&self.out.lifetime, recv_mutable(&self.receiver)) {
                (Some(lt), true) => {
                    quote! {
                        fn #name<#lt>(&#lt mut self) -> &#lt mut ::core::mem::MaybeUninit<#ty> {
                            // SAFETY:
                            // We shorten the lifetime of the borrowed data.
                            // The data is not being dropped, and is not accessible
                            // from anywhere else, so this is safe.:w
                            unsafe { std::mem::transmute(&mut self.#name) }
                        }
                    }
                }
                (None, true) => {
                    quote! {
                        fn #name(&mut self) -> &mut ::core::mem::MaybeUninit<#ty> {
                            &mut self.#name
                        }
                    }
                }
                (Some(lt), false) => {
                    quote! {
                        #[allow(clippy::mut_from_ref)]
                        fn #name<#lt>(&#lt self) -> &#lt mut ::core::mem::MaybeUninit<#ty> {
                            // SAFETY:
                            // We mutably alias the underlying cell, which is not very safe, because
                            // it could already be borrowed immutably. However, for this particular case
                            // it is somewhat okay, with emphasis on "somewhat". If this function returns
                            // a constant, this method is safe, because the stack will be overriden with
                            // the exact same data.
                            //
                            // We shorten the lifetime of the borrowed data.
                            // The data is not being dropped, and is not accessible
                            // from anywhere else, so this is safe.:w
                            unsafe { std::mem::transmute(self.#name.as_ptr().as_mut().unwrap()) }
                        }
                    }
                }
                (None, false) => {
                    quote! {
                        #[allow(clippy::mut_from_ref)]
                        fn #name(&self) -> &mut ::core::mem::MaybeUninit<#ty> {
                            // SAFETY:
                            // We mutably alias the underlying cell, which is not very safe, because
                            // it could already be borrowed immutably. However, for this particular case
                            // it is somewhat okay, with emphasis on "somewhat". If this function returns
                            // a constant, this method is safe, because the stack will be overriden with
                            // the exact same data.
                            unsafe { self.#name.as_ptr().as_mut().unwrap() }
                        }
                    }
                }
            };

            stream.extend(gen);
        }
    }

    pub fn vtbl_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            arg.c_args.to_tokens(&mut ret);
        }

        ret
    }

    pub fn vtbl_args_cast(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            arg.c_cast_args.to_tokens(&mut ret);
        }

        ret
    }

    pub fn trait_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.orig_args {
            let arg = match arg {
                FnArg::Typed(pat) => {
                    if let Pat::Ident(PatIdent { ident, .. }) = &*pat.pat {
                        let ty = &*pat.ty;
                        // TODO: handle subpat
                        quote!(#ident: #ty, )
                    } else {
                        quote!(#arg, )
                    }
                }
                FnArg::Receiver(rcv) => {
                    if rcv.reference.is_some() {
                        quote!(#rcv,)
                    } else {
                        quote!(self,)
                    }
                }
            };

            arg.to_tokens(&mut ret);
        }

        ret
    }

    pub fn trait_passthrough_args(&self, skip: usize) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in self.orig_args.iter().skip(skip) {
            match arg {
                FnArg::Typed(ty) => {
                    let pat = &ty.pat;
                    quote!(#pat,).to_tokens(&mut ret);
                }
                FnArg::Receiver(_) => {
                    quote!(self,).to_tokens(&mut ret);
                }
            }
        }

        ret
    }

    pub fn to_c_def_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            arg.to_c_args.to_tokens(&mut ret);
        }

        ret
    }

    pub fn to_c_call_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            arg.call_c_args.to_tokens(&mut ret);
        }

        ret
    }

    pub fn to_trait_call_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            arg.to_trait_arg.to_tokens(&mut ret);
        }

        ret
    }

    /// Create a VTable definition for this function
    pub fn vtbl_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        let unsafety = &self.get_safety();
        let args = self.vtbl_args();
        let ParsedReturnType {
            c_out,
            c_cast_out,
            c_ret_params,
            lifetime,
            lifetime_cast,
            unbounded_hrtb,
            lifetime_map,
            ..
        } = &self.out;

        let ParsedGenerics {
            life_declare: sig_life_declare,
            ..
        } = &self.sig_generics;

        let sig_life_declare = remap_lifetime_defs(sig_life_declare, lifetime_map);

        let (hrtb, args, c_out) = match (
            lifetime.as_ref().filter(|lt| lt.ident != "cglue_a"),
            lifetime_cast,
            *unbounded_hrtb,
        ) {
            (_, Some(lifetime), false) => (quote!(#lifetime), self.vtbl_args_cast(), c_cast_out),
            (Some(lifetime), _, _) => (quote!(#lifetime), args, c_out),
            _ => (quote!(), args, c_out),
        };

        let sig_life_declare = merge_lifetime_declarations(&sig_life_declare, &parse_quote!(#hrtb));

        let gen = quote! {
            #name: for<#sig_life_declare> #unsafety extern "C" fn(#args #c_ret_params) #c_out,
        };

        stream.extend(gen);
    }

    /// Create a VTable definition for this function
    pub fn vtbl_getter_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        let args = self.vtbl_args();
        let ParsedReturnType {
            c_out,
            c_cast_out,
            c_ret_params,
            lifetime,
            lifetime_cast,
            unbounded_hrtb,
            lifetime_map,
            ..
        } = &self.out;

        let ParsedGenerics {
            life_declare: sig_life_declare,
            ..
        } = &self.sig_generics;

        let sig_life_declare = remap_lifetime_defs(sig_life_declare, lifetime_map);

        let (hrtb, args, c_out) = match (
            lifetime.as_ref().filter(|lt| lt.ident != "cglue_a"),
            lifetime_cast,
            *unbounded_hrtb,
        ) {
            (_, Some(lifetime), false) => (quote!(#lifetime), self.vtbl_args_cast(), c_cast_out),
            (Some(lifetime), _, _) => (quote!(#lifetime), args, c_out),
            _ => (quote!(), args, c_out),
        };

        let sig_life_declare = merge_lifetime_declarations(&sig_life_declare, &parse_quote!(#hrtb));

        let doc_text = format!(" Getter for {}.", name);

        let gen = quote! {
            #[doc = #doc_text]
            ///
            /// Note that this function is wrapped into unsafe, because if already were is an
            /// opaque one, it would allow to invoke undefined behaviour.
            pub fn #name(&self) -> for<#sig_life_declare> unsafe extern "C" fn(#args #c_ret_params) #c_out {
                unsafe { ::core::mem::transmute(self.#name) }
            }
        };

        stream.extend(gen);

        if lifetime_cast.is_some() && *unbounded_hrtb {
            let name2 = format_ident!("{}_lifetimed", name);

            let safety = self.get_safety();

            let args_cast = self.vtbl_args_cast();

            let gen = quote! {
                #[doc = #doc_text]
                ///
                /// This function has its argument lifetime cast so that it's usable with anonymous
                /// lifetime functions.
                ///
                /// # Safety
                ///
                /// This ought to only be used when references to objects are being returned,
                /// otherwise there is a risk of lifetime rule breakage.
                unsafe fn #name2(&self) -> for<#lifetime_cast> #safety extern "C" fn(#args_cast #c_ret_params) #c_cast_out {
                    ::core::mem::transmute(self.#name)
                }
            };

            stream.extend(gen);
        }
    }

    pub fn is_wrapped(&self) -> bool {
        true //self.abi == FuncAbi::Wrapped || self.has_nontrivial
    }

    /// Create a wrapper implementation body for this function
    ///
    /// If the function is ReprC already, it will not be wrapped and will return `None`
    pub fn cfunc_def(
        &self,
        tokens: &mut TokenStream,
        trg_path: &TokenStream,
        ret_tmp: &TokenStream,
        assocs: &ParsedGenerics,
        assoc_equality: &TokenStream,
    ) -> Option<&TokenStream> {
        if !self.is_wrapped() {
            return None;
        }

        let name = &self.name;
        let args = self.vtbl_args();
        let ParsedReturnType {
            c_out,
            c_where_bounds,
            c_where_bounds_cast,
            c_ret,
            c_pre_call,
            c_ret_params,
            use_hrtb,
            lifetime,
            lifetime_cast,
            unbounded_hrtb,
            lifetime_map,
            ..
        } = &self.out;
        let call_args = self.to_trait_call_args();

        let trname = &self.trait_name;
        let fnname = format_ident!("{}{}", FN_PREFIX, name);
        let safety = self.get_safety();

        let ParsedGenerics {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds,
            ..
        } = &self.generics;

        let assoc_declare = &assocs.gen_declare;

        let ParsedGenerics {
            life_declare: sig_life_declare,
            ..
        } = &self.sig_generics;

        let sig_life_declare = remap_lifetime_defs(sig_life_declare, lifetime_map);

        let tmp_lifetime = if *use_hrtb && !life_use.is_empty() {
            quote!('cglue_b, )
        } else {
            quote!(#life_use)
        };

        let tmp_lifetime_anon = if *use_hrtb && !life_use.is_empty() {
            quote!('_, )
        } else {
            quote!(#life_use)
        };

        // Inject 'cglue_a if there are no lifetimes declared by the trait,
        // and temp lifetime is needed
        let life_declare = if lifetime.is_none() || !life_declare.is_empty() {
            life_declare.clone()
        } else {
            parse_quote!(#lifetime,)
        };

        let sig_life_declare = merge_lifetime_declarations(&sig_life_declare, &life_declare);

        let mut container_bound = quote!();

        let (c_pre_call, cglue_c_into_inner) = if !recv_reference(&self.receiver) {
            container_bound.extend(quote!(#trg_path::CGlueObjBase<Context = CGlueCtx> + ));

            (
                quote! {
                    let (this, cglue_ctx) = cont.cobj_base_owned();
                    let this = unsafe { #trg_path::IntoInner::into_inner(this) };
                    #c_pre_call
                },
                Some(quote!(
                    CGlueC::InstType: #trg_path::IntoInner<InnerTarget = CGlueC::ObjType>,
                )),
            )
        } else if recv_mutable(&self.receiver) {
            let cobj_func = if recv_pin(&self.receiver) {
                quote!(cobj_pin_mut)
            } else {
                quote!(cobj_mut)
            };
            container_bound.extend(quote!(#trg_path::CGlueObjMut<#ret_tmp, Context = CGlueCtx> + ));
            (
                quote! {
                    let (this, ret_tmp, cglue_ctx) = cont.#cobj_func();
                    #c_pre_call
                },
                None,
            )
        } else {
            let cobj_func = if recv_pin(&self.receiver) {
                quote!(cobj_pin_ref)
            } else {
                quote!(cobj_ref)
            };
            container_bound.extend(quote!(#trg_path::CGlueObjRef<#ret_tmp, Context = CGlueCtx> + ));
            (
                quote! {
                    let (this, ret_tmp, cglue_ctx) = cont.#cobj_func();
                    #c_pre_call
                },
                None,
            )
        };

        let inner_impl = if let Some(body) = self.custom_conv.c_inner_body.as_ref() {
            body.clone()
        } else {
            quote!(<CGlueC::ObjType as #trname<#tmp_lifetime_anon #gen_use>>::#name(this, #call_args))
        };

        let c_where_bounds = if lifetime_cast.is_some() && *unbounded_hrtb {
            c_where_bounds_cast
        } else {
            c_where_bounds
        };

        let ctx_bound = super::traits::ctx_bound();

        let gen = quote! {
            #safety extern "C" fn #fnname<#sig_life_declare CGlueC: #container_bound, CGlueCtx: #ctx_bound, #gen_declare #assoc_declare>(#args #c_ret_params) #c_out where #gen_where_bounds #c_where_bounds #cglue_c_into_inner CGlueC::ObjType: for<'cglue_b> #trname<#tmp_lifetime #gen_use #assoc_equality>, {
                #c_pre_call
                let ret = #inner_impl;
                #c_ret
            }
        };

        tokens.extend(gen);

        Some(c_where_bounds)
    }

    pub fn vtbl_default_def(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let fnname = format_ident!("{}{}", FN_PREFIX, name);
        tokens.extend(quote!(#name: #fnname,));
    }

    pub fn get_safety(&self) -> TokenStream {
        if self.safe {
            quote!()
        } else {
            quote!(unsafe)
        }
    }

    pub fn trait_impl(&self, tokens: &mut TokenStream) -> (bool, bool, bool) {
        if !self.only_c_side {
            let name = &self.name;
            let args = self.trait_args();
            let ParsedReturnType {
                ty: out,
                impl_func_ret,
                c_ret_precall_def,
                c_call_ret_args,
                lifetime_cast,
                unbounded_hrtb,
                ..
            } = &self.out;
            let def_args = self.to_c_def_args();
            let call_args = self.to_c_call_args();
            let safety = self.get_safety();
            let abi = self.abi.prefix();

            let ParsedGenerics {
                life_declare: sig_life_declare,
                gen_declare: sig_gen_declare,
                ..
            } = &self.sig_generics;

            let get_vfunc = if lifetime_cast.is_some() && *unbounded_hrtb {
                let name_lifetimed = format_ident!("{}_lifetimed", name);
                quote!(unsafe { self.get_vtbl().#name_lifetimed() })
            } else {
                quote!(self.get_vtbl().#name)
            };

            let custom_precall_impl = self.custom_conv.pre_call_impl.to_token_stream();

            let impl_func_ret = if let Some(impl_func_ret) = &self.custom_conv.impl_func_ret {
                impl_func_ret
            } else {
                impl_func_ret
            };

            let gen = quote! {
                #[inline(always)]
                #safety #abi fn #name <#sig_life_declare #sig_gen_declare> (#args) #out {
                    let __cglue_vfunc = #get_vfunc;
                    #custom_precall_impl
                    #def_args
                    #c_ret_precall_def
                    let mut ret = __cglue_vfunc(#call_args #c_call_ret_args);
                    #impl_func_ret
                }
            };

            tokens.extend(gen);
        }

        (
            recv_mutable(&self.receiver),
            !recv_reference(&self.receiver),
            self.out.return_self,
        )
    }

    pub fn forward_wrapped_trait_impl(&self, tokens: &mut TokenStream) -> bool {
        if !recv_reference(&self.receiver) {
            return false;
        }

        let name = &self.name;
        let args = self.trait_args();
        let passthrough_args = self.trait_passthrough_args(1);
        let ParsedReturnType {
            ty: out, use_wrap, ..
        } = &self.out;
        let safety = self.get_safety();
        let abi = self.abi.prefix();

        let ParsedGenerics {
            life_declare,
            gen_declare,
            ..
        } = &self.sig_generics;

        let return_out = if *use_wrap {
            quote!(Self(ret))
        } else {
            quote!(ret)
        };

        let gen = quote! {
            #[inline(always)]
            #safety #abi fn #name <#life_declare #gen_declare> (#args) #out {
                let ret = (self.0).#name(#passthrough_args);
                #return_out
            }
        };

        tokens.extend(gen);

        recv_mutable(&self.receiver)
    }

    pub fn arc_wrapped_trait_impl(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let args = self.trait_args();
        let passthrough_args = self.trait_passthrough_args(1);
        let ParsedReturnType {
            ty: out, use_wrap, ..
        } = &self.out;
        let safety = self.get_safety();
        let abi = self.abi.prefix();

        let ParsedGenerics {
            life_declare,
            gen_declare,
            ..
        } = &self.sig_generics;

        let get_inner = if !recv_reference(&self.receiver) {
            quote!(self.into_inner())
        } else if recv_mutable(&self.receiver) {
            quote!(self.as_mut())
        } else {
            quote!(self.as_ref())
        };

        let return_out = if *use_wrap {
            quote!((ret, arc).into())
        } else {
            quote!(ret)
        };

        let gen = quote! {
            #[inline(always)]
            #safety #abi fn #name <#life_declare #gen_declare> (#args) #out {
                let (inner, arc) = #get_inner;
                let ret = inner.#name(#passthrough_args);
                #return_out
            }
        };

        tokens.extend(gen);
    }

    pub fn int_trait_impl(
        &self,
        ext_path: Option<&Path>,
        ext_name: &Ident,
        tokens: &mut TokenStream,
    ) {
        let name = &self.name;
        let args = self.trait_args();
        let passthrough_args = self.trait_passthrough_args(0);
        let ParsedReturnType { ty: out, .. } = &self.out;
        let safety = self.get_safety();
        let abi = self.abi.prefix();

        let gen = quote! {
            #[inline(always)]
            #safety #abi fn #name (#args) #out {
                #ext_path #ext_name::#name(#passthrough_args)
            }
        };

        tokens.extend(gen);
    }
}

#[derive(Debug, Eq, PartialEq)]
enum FuncAbi {
    ReprC,
    Wrapped,
}

impl FuncAbi {
    pub fn prefix(&self) -> TokenStream {
        match self {
            FuncAbi::ReprC => quote!(extern "C"),
            FuncAbi::Wrapped => quote!(),
        }
    }
}

impl From<Option<Abi>> for FuncAbi {
    fn from(abi: Option<Abi>) -> Self {
        // If the function uses C abi, we can omit wrapper functions
        if let Some(Abi {
            extern_token: _,
            name: Some(abi),
        }) = abi
        {
            if abi.value() == "C" {
                return FuncAbi::ReprC;
            }
        }

        FuncAbi::Wrapped
    }
}

struct ParsedReturnType {
    ty: ReturnType,
    c_out: TokenStream,
    c_cast_out: TokenStream,
    c_where_bounds: TokenStream,
    c_where_bounds_cast: TokenStream,
    c_ret_params: TokenStream,
    c_ret_precall_def: TokenStream,
    c_call_ret_args: TokenStream,
    c_pre_call: Option<TokenStream>,
    c_ret: TokenStream,
    impl_func_ret: TokenStream,
    /// Whether HRTB and tmp stack should be injected.
    ///
    /// HRTB is the `for<'cglue_b>` bound to bind `this` lifetime to be the same one as another
    /// argument's, as well as the return type's. This is only relevant when tmp_ret is being
    /// used. In addition to that, generic bounds will be added to the C wrapper for equivalency.
    injected_ret_tmp: Option<GenericType>,
    injected_ret_tmp_static: Option<GenericType>,
    use_hrtb: bool,
    lifetime: Option<Lifetime>,
    lifetime_cast: Option<Lifetime>,
    unbounded_hrtb: bool,
    return_self: bool,
    use_wrap: bool,
    // Map in-function lifetimes to type lifetimes
    lifetime_map: BTreeMap<Lifetime, Lifetime>,
}

// TODO: handle more cases
#[allow(clippy::single_match)]
fn wrapped_lifetime(mut ty: Type, target: Lifetime) -> Type {
    match &mut ty {
        Type::Path(path) => {
            for seg in path.path.segments.iter_mut() {
                if let PathArguments::AngleBracketed(args) = &mut seg.arguments {
                    for arg in args.args.iter_mut() {
                        match arg {
                            GenericArgument::Lifetime(lt) => {
                                if lt.ident != format_ident!("static") {
                                    *lt = target.clone()
                                }
                            }
                            GenericArgument::Type(ty) => {
                                *ty = wrapped_lifetime(ty.clone(), target.clone())
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        _ => {}
    }
    ty
}

impl ParsedReturnType {
    #[allow(clippy::never_loop)]
    fn new(
        (ty, c_override): (ReturnType, Option<&ReturnType>),
        targets: &BTreeMap<Option<AssocType>, WrappedType>,
        res_override: Option<&Ident>,
        int_result: bool,
        unsafety: &TokenStream,
        (func_name, receiver): (&Ident, &FnArg),
        (crate_path, trait_name, trait_generics): (&TokenStream, &Ident, &ParsedGenerics),
    ) -> Self {
        let mut c_ty = c_override.unwrap_or(&ty).clone();

        let mut ret = Self {
            c_out: ty.to_token_stream(),
            c_cast_out: ty.to_token_stream(),
            ty,
            c_where_bounds: quote!(),
            c_where_bounds_cast: quote!(),
            c_ret_params: quote!(),
            c_ret_precall_def: quote!(),
            c_call_ret_args: quote!(),
            c_pre_call: None,
            c_ret: quote!(ret),
            impl_func_ret: quote!(ret),
            injected_ret_tmp: None,
            injected_ret_tmp_static: None,
            use_hrtb: false,
            lifetime: None,
            lifetime_cast: None,
            unbounded_hrtb: false,
            return_self: false,
            use_wrap: false,
            lifetime_map: Default::default(),
        };

        if let ReturnType::Type(_, ty) = &mut c_ty {
            let mut ty_cast = None;

            // If this branch is hit (whenever ty includes any of the targets), then return type is
            // replaced with c_ty. However, we need to do that regardless in custom impl.
            if let Some(wrapped) = ret_wrap_type(&mut *ty, targets) {
                let old_ty = wrapped.0;
                let trait_ty = wrapped.1;
                let WrappedType {
                    return_conv,
                    lifetime_bound,
                    lifetime_type_bound,
                    other_bounds,
                    other_bounds_simple,
                    inject_ret_tmp,
                    impl_return_conv,
                    ty_static,
                    ty_ret_tmp,
                    ty:
                        GenericType {
                            generic_lifetimes: old_lifetimes,
                            ..
                        },
                    ..
                } = wrapped.2;

                // Swap to check if trait_ty even exists before cloning
                ret.lifetime_map = trait_ty
                    .iter()
                    .flat_map(|assoc| assoc.generics.lifetimes().map(|v| &v.lifetime).cloned())
                    .zip(old_lifetimes.iter().cloned())
                    .collect();

                // TODO: sort out the order

                let (mutable, lifetime) = match (inject_ret_tmp, &**ty) {
                    (true, Type::Reference(ty)) => {
                        (ty.mutability.is_some(), ty.lifetime.as_ref().cloned())
                    }
                    (false, _) => (false, None),
                    _ => {
                        panic!(
                            "Wrapped ref return currently only valid for references! (ty: {ty})",
                            ty = ty.to_token_stream()
                        )
                    }
                };

                let unbounded_hrtb = lifetime.is_none() && lifetime_type_bound.is_none();

                let cglue_b_lifetime = Lifetime {
                    apostrophe: proc_macro2::Span::call_site(),
                    ident: format_ident!("cglue_b"),
                };

                let lifetime = lifetime.or_else(|| lifetime_bound.clone()).or_else(|| {
                    Some(Lifetime {
                        apostrophe: proc_macro2::Span::call_site(),
                        ident: format_ident!("cglue_a"),
                    })
                });

                let lifetime_cast = if lifetime
                    .as_ref()
                    .filter(|lt| lt.ident == "cglue_a")
                    .is_some()
                {
                    ty_cast = Some(ty.clone());
                    Some(cglue_b_lifetime.clone())
                } else {
                    None
                };

                if let Some(lifetime) = &lifetime {
                    **ty = wrapped_lifetime(*ty.clone(), lifetime.clone());
                }

                // TODO: should this inherit lifetime, or just fallback on lifetime?
                let lifetime_type_bound = lifetime_type_bound.clone().map(|lt| {
                    if lt.ident != "static" {
                        cglue_b_lifetime
                    } else {
                        lt
                    }
                });

                let ret_wrap = match return_conv {
                    Some(conv) => quote! {
                        let mut conv = #conv;
                        conv(ret)
                    },
                    _ => quote!(ret.into()),
                };

                let life_use = &trait_generics.life_use;
                let gen_use = &trait_generics.gen_use;

                // TODO: where do we need this bound?

                let is_static = lifetime_type_bound.map(|l| l.ident == "static") == Some(true);

                let (static_bound, static_bound_simple) = if is_static && trait_ty.is_some() {
                    if life_use.is_empty() {
                        (
                            quote!(for<'cglue_b> <CGlueC::ObjType as #trait_name<#gen_use>>::#trait_ty: 'static,),
                            quote!(for<'cglue_b> <CGlueC::ObjType as #trait_name<#gen_use>>::#trait_ty: 'static,),
                        )
                    } else {
                        (
                            quote!(for<'cglue_b> <CGlueC::ObjType as #trait_name<'cglue_b, #gen_use>>::#trait_ty: 'static,),
                            quote!(<CGlueC::ObjType as #trait_name<'cglue_a, #gen_use>>::#trait_ty: 'static,),
                        )
                    }
                } else {
                    (quote!(), quote!())
                };

                let where_bound = quote!(#static_bound #other_bounds);
                let where_bound_simple = quote!(#static_bound_simple #other_bounds_simple);

                // Replace the lifetime of the type.
                if let (true, Type::Reference(ty)) = (inject_ret_tmp, &mut **ty) {
                    ty.lifetime = lifetime.clone();

                    if let Some(ty_cast) = &mut ty_cast {
                        if let Type::Reference(ty_cast) = &mut **ty_cast {
                            ty_cast.lifetime = lifetime_cast.clone();
                        }
                    }
                };

                let (injected_ret_tmp, c_pre_call) = match (inject_ret_tmp, mutable) {
                    (true, false) => (
                        ty_ret_tmp.clone(),
                        quote!(let ret_tmp = ret_tmp.#func_name();),
                    ),
                    (true, true) => (
                        ty_ret_tmp.clone(),
                        quote!(let ret_tmp = ret_tmp.#func_name();),
                    ),
                    _ => (None, quote!()),
                };

                let c_pre_call = if recv_reference(receiver) {
                    quote!(#c_pre_call let cglue_ctx = cglue_ctx.clone();)
                } else {
                    c_pre_call
                };

                let impl_return_conv = impl_return_conv
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| quote!(ret));

                let return_self = trait_ty.map(|i| i.ident == "Self") == Some(true);

                ret.c_out = quote!(-> #ty);
                ret.c_cast_out = quote!(-> #ty_cast);

                // If we are returning self, do not actually change the return type.
                // I know, messy :(
                if return_self {
                    *ty = Box::new(old_ty);
                }

                ret.c_where_bounds = where_bound;
                ret.c_where_bounds_cast = where_bound_simple;
                ret.c_ret_params = quote!();
                ret.c_ret_precall_def = quote!();
                ret.c_call_ret_args = quote!();
                ret.c_pre_call = Some(c_pre_call);
                ret.c_ret = quote!(#ret_wrap);
                ret.impl_func_ret = impl_return_conv;
                ret.injected_ret_tmp = injected_ret_tmp;
                ret.injected_ret_tmp_static = ty_static.clone();
                ret.use_hrtb = true;
                ret.unbounded_hrtb = unbounded_hrtb;
                ret.return_self = return_self;
                ret.use_wrap = true;
                ret.lifetime = lifetime;
                ret.lifetime_cast = lifetime_cast;
            } else {
                ret.c_out = quote!(-> #ty);
            }

            match &mut **ty {
                Type::Reference(r) => {
                    let is_mut = r.mutability.is_some();

                    let mut new_tys = vec![];

                    let default_lt = Lifetime {
                        apostrophe: proc_macro2::Span::call_site(),
                        ident: format_ident!("_"),
                    };

                    for r in Some(&*r)
                        .into_iter()
                        .chain(ty_cast.as_deref().into_iter().filter_map(|r| {
                            if let Type::Reference(r) = r {
                                Some(r)
                            } else {
                                None
                            }
                        }))
                    {
                        let lt = r.lifetime.as_ref().unwrap_or(&default_lt);

                        new_tys.push(match &*r.elem {
                            Type::Slice(s) => {
                                let ty = &*s.elem;
                                Some(if is_mut {
                                    quote!(#crate_path::slice::CSliceMut<#lt, #ty>)
                                } else {
                                    quote!(#crate_path::slice::CSliceRef<#lt, #ty>)
                                })
                                .map(|v| (v, false))
                            }
                            Type::Path(p) => if let Some("str") =
                                p.path.get_ident().map(|i| i.to_string()).as_deref()
                            {
                                Some(if is_mut {
                                    quote!(#crate_path::slice::CSliceMut<#lt, u8>)
                                } else {
                                    quote!(#crate_path::slice::CSliceRef<#lt, u8>)
                                })
                            } else {
                                None
                            }
                            .map(|v| (v, true)),
                            _ => None,
                        });
                    }

                    if let Some((slty, into_str)) = &new_tys[0] {
                        ret.c_out = quote!(-> #slty);
                        if let Some(sltyc) = new_tys.get(1) {
                            let sltyc = sltyc.as_ref().map(|(v, _)| v);
                            ret.c_cast_out = quote!(-> #sltyc);
                        }
                        ret.c_ret.extend(quote!(.into()));
                        if *into_str {
                            let old_ret = ret.impl_func_ret;
                            ret.impl_func_ret = quote! {
                                unsafe { #old_ret .into_str() }
                            }
                        } else {
                            ret.impl_func_ret.extend(quote!(.into()));
                        }
                    }
                }
                Type::Path(p) => {
                    let last = p.path.segments.last();
                    if let Some((PathArguments::AngleBracketed(args), last)) =
                        last.map(|l| (&l.arguments, l))
                    {
                        let ident = &last.ident;

                        let result_ident = format_ident!("Result");

                        let result_ident = res_override.unwrap_or(&result_ident);

                        match ident.to_string().as_str() {
                            "Option" => {
                                if let Some(GenericArgument::Type(a)) = args.args.first() {
                                    if !crate::util::is_null_pointer_optimizable(a, &[]) {
                                        let new_path: Path =
                                            parse2(quote!(#crate_path::option::COption))
                                                .expect("Failed to parse COption path");

                                        replace_path_keep_final_args(
                                            Some(&mut **ty),
                                            new_path.clone(),
                                        );
                                        replace_path_keep_final_args(
                                            ty_cast.as_deref_mut(),
                                            new_path,
                                        );

                                        ret.c_out = quote!(-> #ty);
                                        ret.c_cast_out = quote!(-> #ty_cast);
                                        ret.c_ret.extend(quote!(.into()));
                                        ret.impl_func_ret.extend(quote!(.into()));
                                    }
                                }
                            }
                            _ => {
                                if ident == result_ident {
                                    let mut args = args.args.iter();

                                    let to_match =
                                        (args.next(), args.next(), args.next(), int_result);

                                    std::mem::drop(args);

                                    match to_match {
                                        (Some(GenericArgument::Type(a)), _, None, true) => loop {
                                            ret.c_out = quote!(-> i32);
                                            ret.c_cast_out = quote!(-> i32);

                                            let c_ret = &ret.c_ret;

                                            let mapped_ret = quote! {
                                                let ret = ret.map(|ret| {
                                                    #c_ret
                                                });
                                            };

                                            if let Type::Tuple(tup) = a {
                                                if tup.elems.is_empty() {
                                                    ret.c_ret = quote! {
                                                        #mapped_ret
                                                        #crate_path::result::into_int_result(ret)
                                                    };
                                                    let impl_func_ret = &ret.impl_func_ret;
                                                    ret.impl_func_ret = quote!(#crate_path::result::from_int_result_empty(#impl_func_ret));

                                                    break;
                                                }
                                            }

                                            ret.c_ret_params.extend(
                                                quote!(ok_out: &mut ::core::mem::MaybeUninit<#a>,),
                                            );
                                            ret.c_ret_precall_def.extend(quote!(let mut ok_out = ::core::mem::MaybeUninit::uninit();));
                                            ret.c_call_ret_args.extend(quote!(&mut ok_out,));

                                            ret.c_ret = quote! {
                                                #mapped_ret
                                                #crate_path::result::into_int_out_result(ret, ok_out)
                                            };
                                            let impl_func_ret = &ret.impl_func_ret;
                                            ret.impl_func_ret = quote!(#unsafety { #crate_path::result::from_int_result(#impl_func_ret, ok_out) });

                                            break;
                                        },
                                        (
                                            Some(GenericArgument::Type(_)),
                                            Some(GenericArgument::Type(_)),
                                            None,
                                            _,
                                        ) => {
                                            let new_path: Path =
                                                parse2(quote!(#crate_path::result::CResult))
                                                    .expect("Failed to parse CResult path");

                                            replace_path_keep_final_args(
                                                Some(&mut **ty),
                                                new_path.clone(),
                                            );
                                            replace_path_keep_final_args(
                                                ty_cast.as_deref_mut(),
                                                new_path,
                                            );

                                            ret.c_out = quote!(-> #ty);
                                            ret.c_cast_out = quote!(-> #ty_cast);

                                            let c_ret = &ret.c_ret;

                                            let mapped_ret = quote! {
                                                let ret = ret.map(|ret| {
                                                    #c_ret
                                                });
                                            };

                                            ret.c_ret = quote! {
                                                #mapped_ret
                                                ret.into()
                                            };

                                            ret.impl_func_ret.extend(quote!(.into()));
                                        }
                                        _ => {}
                                    };
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        ret
    }
}

fn replace_path_keep_final_args(ty: Option<&mut Type>, new_path: Path) {
    if let Some(Type::Path(path)) = ty {
        let old_path = std::mem::replace(&mut path.path, new_path);
        if let Some(seg) = old_path.segments.into_iter().last() {
            if let Some(new_seg) = path.path.segments.iter_mut().last() {
                new_seg.arguments = seg.arguments;
            }
        }
    }
}
