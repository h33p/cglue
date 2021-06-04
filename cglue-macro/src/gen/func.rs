use super::generics::{GenericType, ParsedGenerics};
use proc_macro2::TokenStream;
use quote::*;
use std::collections::BTreeMap;
use syn::{Type, *};

const FN_PREFIX: &str = "cglue_wrapped_";

pub struct WrappedType {
    pub ty: GenericType,
    pub lifetime_bound: Option<Lifetime>,
    pub other_bounds: Option<TokenStream>,
    pub return_conv: Option<ExprClosure>,
    pub inject_ret_tmp: bool,
}

/// TraitArg stores implementations for Unstable-C-Unstable ABI transitions.
struct TraitArg {
    arg: FnArg,
    /// Called in trait impl to define arguments. Useful when need to destruct a tuple/struct.
    to_c_args: TokenStream,
    /// Arguments inside the call to the C vtable function.
    call_c_args: TokenStream,
    /// C function signature.
    c_args: TokenStream,
    /// Arguments inside the call to the trait function.
    to_trait_arg: TokenStream,
    /// Whether argument conversion is trivial - 1-1 relationship with no changed types
    trivial: bool,
}

fn wrap_type<'a>(
    ty: &mut Type,
    targets: &'a BTreeMap<Ident, WrappedType>,
) -> Option<(Type, Ident, &'a WrappedType)> {
    match ty {
        Type::Reference(r) => wrap_type(&mut *r.elem, targets),
        Type::Slice(s) => wrap_type(&mut *s.elem, targets),
        Type::Path(p) => {
            let mut iter = p.path.segments.iter();
            if let (None, None, Some(p1), Some(p2)) =
                (&p.qself, p.path.leading_colon, iter.next(), iter.next())
            {
                if p1.ident == "Self" {
                    if let Some(wrapped) = targets.get(&p2.ident) {
                        let WrappedType { ty: new_ty, .. } = wrapped;

                        std::mem::drop(iter);

                        let ident = p2.ident.clone();

                        let ret = std::mem::replace(
                            ty,
                            syn::parse2(new_ty.to_token_stream())
                                .expect("Failed to parse wrap_type"),
                        );

                        return Some((ret, ident, wrapped));
                    }
                }
            }

            None
        }
        Type::Ptr(ptr) => wrap_type(&mut *ptr.elem, targets),
        Type::Tuple(tup) => tup
            .elems
            .iter_mut()
            .filter_map(|e| wrap_type(e, targets))
            .next(),
        // TODO: Other types
        _ => None,
    }
}

impl TraitArg {
    fn new(
        mut arg: FnArg,
        targets: &BTreeMap<Ident, WrappedType>,
        unsafety: &TokenStream,
        crate_path: &TokenStream,
        inject_hrtb: bool,
    ) -> Self {
        let (to_c_args, call_c_args, c_args, to_trait_arg, trivial) = match &mut arg {
            FnArg::Receiver(r) => {
                let lifetime = if inject_hrtb {
                    quote!('cglue_b )
                } else {
                    quote!()
                };

                if r.mutability.is_some() {
                    (
                        quote!(let (this, ret_tmp) = self.cobj_mut();),
                        quote!(this,),
                        quote!(this: &#lifetime mut CGlueT,),
                        quote!(),
                        true,
                    )
                } else {
                    (
                        quote!(let (this, ret_tmp) = self.cobj_ref();),
                        quote!(this,),
                        quote!(this: &#lifetime CGlueT,),
                        quote!(),
                        true,
                    )
                }
            }
            FnArg::Typed(t) => {
                let _old = wrap_type(&mut *t.ty, targets);

                let name = &*t.pat;
                let ty = &*t.ty;

                let mut ret = None;

                // TODO: deal with nested conversion
                //if let (Some(old), Type::Path(p)) = {
                //}

                match ty {
                    Type::Reference(r) => {
                        let is_mut = r.mutability.is_some();
                        if let Type::Slice(s) = &*r.elem {
                            let szname =
                                format_ident!("{}_size", name.to_token_stream().to_string());
                            let ty = &*s.elem;
                            let (as_ptr, from_raw_parts, ptrt) = if is_mut {
                                (
                                    quote!(as_mut_ptr),
                                    quote!(from_raw_parts_mut),
                                    quote!(*mut #ty),
                                )
                            } else {
                                (quote!(as_ptr), quote!(from_raw_parts), quote!(*const #ty))
                            };

                            ret = Some((
                                quote!(let (#name, #szname) = (#name.#as_ptr(), #name.len());),
                                quote!(#name, #szname,),
                                quote!(#name: #ptrt, #szname: usize,),
                                quote!(#unsafety { ::core::slice::#from_raw_parts(#name, #szname) },),
                                false,
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
                                                quote!(#name.into(),),
                                                false,
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
                                                quote!(#name.into(),),
                                                false,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
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
                        quote!(#name,),
                        true,
                    )
                })
            }
        };

        Self {
            arg,
            to_c_args,
            call_c_args,
            c_args,
            to_trait_arg,
            trivial,
        }
    }
}

pub struct ParsedFunc {
    name: Ident,
    trait_name: Ident,
    safe: bool,
    abi: FuncAbi,
    has_nontrivial: bool,
    receiver: Receiver,
    args: Vec<TraitArg>,
    out: ParsedReturnType,
    generics: ParsedGenerics,
}

impl ParsedFunc {
    pub fn new(
        sig: Signature,
        trait_name: Ident,
        generics: &ParsedGenerics,
        wrap_types: &BTreeMap<Ident, WrappedType>,
        int_result: bool,
        crate_path: &TokenStream,
    ) -> Option<Self> {
        let name = sig.ident;
        let safe = sig.unsafety.is_none();
        let abi = From::from(sig.abi);
        let mut args: Vec<TraitArg> = vec![];

        let mut receiver = None;
        let mut has_nontrivial = false;

        let unsafety = if safe { quote!(unsafe) } else { quote!() };

        let out = ParsedReturnType::new(
            sig.output, wrap_types, int_result, &unsafety, &name, crate_path,
        );

        for input in sig.inputs.into_iter() {
            if let FnArg::Receiver(r) = &input {
                receiver = Some(r.clone());
            }

            let func = TraitArg::new(
                input,
                wrap_types,
                &unsafety,
                crate_path,
                out.injected_ret_tmp.is_some(),
            );

            has_nontrivial = has_nontrivial || !func.trivial;

            args.push(func);
        }

        let receiver = receiver?;

        let generics = generics.clone();

        Some(Self {
            name,
            trait_name,
            safe,
            abi,
            has_nontrivial,
            receiver,
            args,
            out,
            generics,
        })
    }

    pub fn ret_tmp_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        if let Some(ty) = &self.out.injected_ret_tmp {
            let gen = if self.receiver.mutability.is_some() {
                quote!(#name: ::core::mem::MaybeUninit<#ty>,)
            } else {
                quote!(#name: ::core::cell::Cell<::core::mem::MaybeUninit<#ty>>,)
            };
            stream.extend(gen);
        }
    }

    pub fn ret_default_def(&self, stream: &mut TokenStream) {
        let name = &self.name;
        if let Some(_) = &self.out.injected_ret_tmp {
            let gen = if self.receiver.mutability.is_some() {
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
            let gen = if self.receiver.mutability.is_some() {
                quote! {
                    fn #name(&mut self) -> &mut ::core::mem::MaybeUninit<#ty> {
                        &mut self.#name
                    }
                }
            } else {
                quote! {
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

    pub fn trait_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for arg in &self.args {
            let arg = &arg.arg;
            let arg = quote!(#arg, );
            arg.to_tokens(&mut ret);
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
        let args = self.vtbl_args();
        let ParsedReturnType {
            c_out,
            c_ret_params,
            injected_ret_tmp,
            ..
        } = &self.out;

        let hrtb = if injected_ret_tmp.is_some() {
            quote!(for<'cglue_b> )
        } else {
            quote!()
        };

        let gen = quote! {
            pub #name: #hrtb extern "C" fn(#args #c_ret_params) #c_out,
        };

        stream.extend(gen);
    }

    pub fn is_wrapped(&self) -> bool {
        self.abi == FuncAbi::Wrapped || self.has_nontrivial
    }

    /// Create a wrapper implementation body for this function
    ///
    /// If the function is ReprC already, it will not be wrapped and will return `None`
    pub fn cfunc_def(&self, tokens: &mut TokenStream) {
        if !self.is_wrapped() {
            return;
        }

        let name = &self.name;
        let args = self.vtbl_args();
        let ParsedReturnType {
            c_out,
            c_where_bounds,
            c_ret,
            c_ret_params,
            injected_ret_tmp,
            ..
        } = &self.out;
        let call_args = self.to_trait_call_args();

        let trname = &self.trait_name;
        let fnname = format_ident!("{}{}", FN_PREFIX, name);
        let safety = self.get_safety();

        let tmp_lifetime = if injected_ret_tmp.is_some() {
            quote!('cglue_b, )
        } else {
            quote!()
        };

        let ParsedGenerics {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds,
            ..
        } = &self.generics;

        let gen = quote! {
            #safety extern "C" fn #fnname<#tmp_lifetime #life_declare CGlueT: #trname<#life_use #gen_use>, #gen_declare>(#args #c_ret_params) #c_out where #gen_where_bounds #c_where_bounds {
                let ret = this.#name(#call_args);
                #c_ret
            }
        };

        tokens.extend(gen);
    }

    pub fn vtbl_default_def(&self, tokens: &mut TokenStream) {
        let name = &self.name;

        let fnname: TokenStream = if self.is_wrapped() {
            format!("{}{}", FN_PREFIX, name)
        } else {
            format!("CGlueT::{}", name)
        }
        .parse()
        .unwrap();

        tokens.extend(quote!(#name: #fnname,));
    }

    pub fn get_safety(&self) -> TokenStream {
        if self.safe {
            quote!()
        } else {
            quote!(unsafe)
        }
    }

    pub fn trait_impl(&self, tokens: &mut TokenStream) -> bool {
        let name = &self.name;
        let args = self.trait_args();
        let ParsedReturnType {
            ty: out,
            impl_func_ret,
            c_ret_precall_def,
            c_call_ret_args,
            ..
        } = &self.out;
        let def_args = self.to_c_def_args();
        let call_args = self.to_c_call_args();
        let safety = self.get_safety();
        let abi = self.abi.prefix();

        let gen = quote! {
            #[inline(always)]
            #safety #abi fn #name (#args) #out {
                let __cglue_vfunc = self.vtbl_ref().#name;
                #def_args
                #c_ret_precall_def
                let ret = __cglue_vfunc(#call_args #c_call_ret_args);
                #impl_func_ret
            }
        };

        tokens.extend(gen);

        self.receiver.mutability.is_some()
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
    c_where_bounds: TokenStream,
    c_ret_params: TokenStream,
    c_ret_precall_def: TokenStream,
    c_call_ret_args: TokenStream,
    c_ret: TokenStream,
    impl_func_ret: TokenStream,
    /// Whether HRTB and tmp stack should be injected.
    ///
    /// HRTB is the `for<'cglue_b>` bound to bind `this` lifetime to be the same one as another
    /// argument's, as well as the return type's. This is only relevant when tmp_ret is being
    /// used. In addition to that, generic bounds will be added to the C wrapper for equivalency.
    injected_ret_tmp: Option<GenericType>,
}

impl ParsedReturnType {
    #[allow(clippy::never_loop)]
    fn new(
        mut ty: ReturnType,
        targets: &BTreeMap<Ident, WrappedType>,
        int_result: bool,
        unsafety: &TokenStream,
        func_name: &Ident,
        crate_path: &TokenStream,
    ) -> Self {
        let (
            c_out,
            c_where_bounds,
            c_ret_params,
            c_ret_precall_def,
            c_call_ret_args,
            c_ret,
            impl_func_ret,
            injected_ret_tmp,
        ) = {
            let mut ret = None;

            if let ReturnType::Type(_, ty) = &mut ty {
                if let Some((
                    _,
                    trait_ty,
                    WrappedType {
                        return_conv,
                        lifetime_bound,
                        other_bounds,
                        inject_ret_tmp,
                        ty: new_ty,
                        ..
                    },
                )) = wrap_type(&mut *ty, targets)
                {
                    let mutable = match (*inject_ret_tmp, &**ty) {
                        (true, Type::Reference(ty)) => ty.mutability.is_some(),
                        (false, _) => false,
                        _ => panic!("Wrapped ref return currently only valid for references!"),
                    };

                    let ret_wrap = match return_conv {
                        Some(conv) => quote! {
                            let mut conv = #conv;
                            conv(ret)
                        },
                        _ => quote!(ret.into()),
                    };

                    let where_bound = match lifetime_bound {
                        Some(bound) => quote!(CGlueT::#trait_ty: #bound, #other_bounds),
                        _ => quote!(#other_bounds),
                    };

                    let (ret_type, injected_ret_tmp, tmp_type_def, tmp_impl_def, tmp_call_def) =
                        match (inject_ret_tmp, mutable) {
                            (true, false) => (
                                quote!(&'cglue_b #new_ty),
                                Some(new_ty.clone()),
                                quote!(ret_tmp: &'cglue_b mut ::core::mem::MaybeUninit<#new_ty>,),
                                quote!(let ret_tmp = ret_tmp.#func_name();),
                                quote!(ret_tmp,),
                            ),
                            (true, true) => (
                                quote!(&'cglue_b mut #new_ty),
                                Some(new_ty.clone()),
                                quote!(ret_tmp: &mut ::core::mem::MaybeUninit<#new_ty>,),
                                quote!(let ret_tmp = ret_tmp.#func_name();),
                                quote!(ret_tmp,),
                            ),
                            _ => (quote!(#ty), None, quote!(), quote!(), quote!()),
                        };

                    ret = Some((
                        quote!(-> #ret_type),
                        where_bound,
                        tmp_type_def,
                        tmp_impl_def,
                        tmp_call_def,
                        quote!(#ret_wrap),
                        quote!(ret),
                        injected_ret_tmp,
                    ));
                } else if let Type::Path(p) = &**ty {
                    let last = p.path.segments.last();
                    if let Some((PathArguments::AngleBracketed(args), last)) =
                        last.map(|l| (&l.arguments, l))
                    {
                        match last.ident.to_string().as_str() {
                            "Option" => {
                                if let Some(GenericArgument::Type(a)) = args.args.first() {
                                    if !crate::util::is_null_pointer_optimizable(a, &[]) {
                                        ret = Some((
                                            quote!(-> #crate_path::option::COption<#a>),
                                            quote!(),
                                            quote!(),
                                            quote!(),
                                            quote!(),
                                            quote!(ret.into()),
                                            quote!(ret.into()),
                                            None,
                                        ));
                                    }
                                }
                            }
                            "Result" => {
                                let mut args = args.args.iter();

                                ret = match (args.next(), args.next(), args.next(), int_result) {
                                    (Some(GenericArgument::Type(a)), _, None, true) => {
                                        let ret = loop {
                                            if let Type::Tuple(tup) = a {
                                                if tup.elems.is_empty() {
                                                    break (
                                                        quote!(-> i32),
                                                        quote!(),
                                                        quote!(),
                                                        quote!(),
                                                        quote!(),
                                                        quote!(#crate_path::result::into_int_result(ret)),
                                                        quote!(#crate_path::result::from_int_result_empty(ret)),
                                                        None,
                                                    );
                                                }
                                            }

                                            break (
                                                quote!(-> i32),
                                                quote!(),
                                                quote!(ok_out: &mut ::core::mem::MaybeUninit<#a>),
                                                quote!(let mut ok_out = ::core::mem::MaybeUninit::uninit();),
                                                quote!(&mut ok_out,),
                                                quote!(#crate_path::result::into_int_out_result(ret, ok_out)),
                                                quote!(#unsafety { #crate_path::result::from_int_result(ret, ok_out) }),
                                                None,
                                            );
                                        };

                                        Some(ret)
                                    }
                                    (
                                        Some(GenericArgument::Type(a)),
                                        Some(GenericArgument::Type(b)),
                                        None,
                                        _,
                                    ) => Some((
                                        quote!(-> #crate_path::result::CResult<#a, #b>),
                                        quote!(),
                                        quote!(),
                                        quote!(),
                                        quote!(),
                                        quote!(ret.into()),
                                        quote!(ret.into()),
                                        None,
                                    )),
                                    _ => None,
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }

            ret.unwrap_or_else(|| {
                (
                    ty.to_token_stream(),
                    quote!(),
                    quote!(),
                    quote!(),
                    quote!(),
                    quote!(ret),
                    quote!(ret),
                    None,
                )
            })
        };

        Self {
            ty,
            c_out,
            c_where_bounds,
            c_ret_params,
            c_ret_precall_def,
            c_call_ret_args,
            c_ret,
            impl_func_ret,
            injected_ret_tmp,
        }
    }
}
