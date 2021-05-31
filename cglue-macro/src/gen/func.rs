use proc_macro2::TokenStream;
use quote::*;
use syn::{Type, *};

const FN_PREFIX: &str = "cglue_wrapped_";

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

impl TraitArg {
    fn new(arg: FnArg, unsafety: &TokenStream, crate_path: &TokenStream) -> Self {
        let (to_c_args, call_c_args, c_args, to_trait_arg, trivial) = match &arg {
            FnArg::Receiver(r) => {
                if r.mutability.is_some() {
                    (
                        quote!(let this = self.cobj_mut();),
                        quote!(this,),
                        quote!(this: &mut T,),
                        quote!(),
                        true,
                    )
                } else {
                    (
                        quote!(let this = self.cobj_ref();),
                        quote!(this,),
                        quote!(this: &T,),
                        quote!(),
                        true,
                    )
                }
            }
            FnArg::Typed(t) => {
                let name = &*t.pat;
                let ty = &*t.ty;

                let ret = match ty {
                    Type::Reference(r) => {
                        let is_mut = r.mutability.is_some();
                        match &*r.elem {
                            Type::Slice(s) => {
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

                                Some((
                                    quote!(let (#name, #szname) = (#name.#as_ptr(), #name.len());),
                                    quote!(#name, #szname,),
                                    quote!(#name: #ptrt, #szname: usize,),
                                    quote!(#unsafety { ::core::slice::#from_raw_parts(#name, #szname) },),
                                    false,
                                ))
                            }
                            _ => None,
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
                                        if crate::util::is_null_pointer_optimizable(a, &[]) {
                                            None
                                        } else {
                                            Some((
                                                quote!(let #name = #name.into();),
                                                quote!(#name,),
                                                quote!(#name: #crate_path::option::COption<#a>,),
                                                quote!(#name.into(),),
                                                false,
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                }
                                "Result" => {
                                    let mut args = args.args.iter();

                                    match (args.next(), args.next(), args.next(), false) {
                                        (Some(GenericArgument::Type(_)), _, None, true) => {
                                            // TODO: Wrap Result<T> alias to use int values if it is marked
                                            // to implement IntResult
                                            None
                                        }
                                        (
                                            Some(GenericArgument::Type(a)),
                                            Some(GenericArgument::Type(b)),
                                            None,
                                            _,
                                        ) => Some((
                                            quote!(let #name = #name.into();),
                                            quote!(#name,),
                                            quote!(#name: #crate_path::result::CResult<#a, #b>,),
                                            quote!(#name.into(),),
                                            false,
                                        )),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

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
}

impl ParsedFunc {
    pub fn new(sig: Signature, trait_name: Ident, crate_path: &TokenStream) -> Option<Self> {
        let name = sig.ident;
        let safe = sig.unsafety.is_none();
        let abi = From::from(sig.abi);
        let mut args: Vec<TraitArg> = vec![];

        let mut receiver = None;
        let mut has_nontrivial = false;

        let unsafety = if safe { quote!(unsafe) } else { quote!() };

        for input in sig.inputs.into_iter() {
            if let FnArg::Receiver(r) = &input {
                receiver = Some(r.clone());
            }

            let func = TraitArg::new(input, &unsafety, crate_path);

            has_nontrivial = has_nontrivial || !func.trivial;

            args.push(func);
        }

        let receiver = receiver?;

        let out = From::from(sig.output);

        Some(Self {
            name,
            trait_name,
            safe,
            abi,
            has_nontrivial,
            receiver,
            args,
            out,
        })
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
        let out = &self.out;

        let gen = quote! {
            pub #name: extern "C" fn(#args) -> #out,
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
        let out = &self.out;
        let call_args = self.to_trait_call_args();

        let trname = &self.trait_name;
        let fnname = format_ident!("{}{}", FN_PREFIX, name);
        let safety = self.get_safety();

        // TODO: add support for writing Ok result to MaybeUninit
        // TODO: add checks for result wrapping
        let gen = quote! {
            #safety extern "C" fn #fnname<T: #trname>(#args) -> #out {
                this.#name(#call_args)
            }
        };

        tokens.extend(gen);
    }

    pub fn vtbl_default_def(&self, tokens: &mut TokenStream) {
        let name = &self.name;

        let fnname: TokenStream = if self.is_wrapped() {
            format!("{}{}", FN_PREFIX, name)
        } else {
            format!("T::{}", name)
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
        let out = &self.out;
        let def_args = self.to_c_def_args();
        let call_args = self.to_c_call_args();
        let safety = self.get_safety();
        let abi = self.abi.prefix();

        let gen = quote! {
            #[inline(always)]
            #safety #abi fn #name (#args) -> #out {
                let __cglue_vfunc = self.as_ref().#name;
                #def_args
                __cglue_vfunc(#call_args)
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

enum ParsedReturnType {
    Nothing,
    Other(Box<Type>),
}

impl From<ReturnType> for ParsedReturnType {
    fn from(ty: ReturnType) -> Self {
        match ty {
            ReturnType::Default => ParsedReturnType::Nothing,
            ReturnType::Type(_, ty) => ParsedReturnType::Other(ty),
        }
    }
}

impl ToTokens for ParsedReturnType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match &self {
            ParsedReturnType::Nothing => tokens.extend(quote! { () }),
            ParsedReturnType::Other(x) => x.to_tokens(tokens),
        }
    }
}
