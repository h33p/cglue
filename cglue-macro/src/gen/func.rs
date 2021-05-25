use proc_macro2::TokenStream;
use quote::*;
use syn::{Type, *};

const FN_PREFIX: &'static str = "cglue_wrapped_";

pub struct ParsedFunc {
    name: Ident,
    trait_name: Ident,
    safe: bool,
    abi: FuncAbi,
    receiver: Option<Receiver>,
    args: Vec<ParsedArg>,
    out: ParsedReturnType,
}

impl ParsedFunc {
    pub fn new(sig: Signature, trait_name: Ident) -> Self {
        let name = sig.ident;
        let safe = sig.unsafety.is_none();
        let abi = From::from(sig.abi);
        let mut args: Vec<ParsedArg> = vec![];

        let mut receiver = None;

        for input in sig.inputs.into_iter() {
            if let FnArg::Receiver(r) = input {
                receiver = Some(r.clone());
            } else {
                args.push(input.into());
            }
        }

        let out = From::from(sig.output);

        Self {
            name,
            trait_name,
            safe,
            abi,
            receiver,
            args,
            out,
        }
    }

    pub fn vtbl_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        if let Some(recv) = &self.receiver {
            let pa = ParsedArg::from_receiver(recv.clone(), "T");
            pa.to_tokens(&mut ret);
        }

        for arg in &self.args {
            arg.to_tokens(&mut ret);
        }

        ret
    }

    pub fn trait_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        if let Some(recv) = &self.receiver {
            ret.extend(quote!(#recv, ));
        }

        for arg in &self.args {
            arg.to_tokens(&mut ret);
        }

        ret
    }

    pub fn chained_call_args(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        ret.extend(self.args.iter().map(|i| {
            let name = &i.name;
            quote!(#name, )
        }));

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
        self.abi == FuncAbi::Wrapped
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

        let trname = &self.trait_name;
        let fnname = format_ident!("{}{}", FN_PREFIX, name);

        // TODO: add support for writing Ok result to MaybeUninit
        // TODO: add checks for result wrapping
        let gen = quote! {
            extern "C" fn #fnname<T: #trname>(#args) -> #out {
                this.#name(/* TODO */)
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

    pub fn trait_impl(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let args = self.trait_args();
        let out = &self.out;
        let call_args = self.chained_call_args();

        let this_arg = match &self.receiver {
            Some(x) => {
                if x.mutability.is_some() {
                    quote!(self.cobj_mut())
                } else {
                    quote!(self.cobj_ref())
                }
            }
            _ => quote!(()),
        };

        let abi = self.abi.prefix();

        let gen = quote! {
            #[inline(always)]
            #abi fn #name (#args) -> #out {
                (self.as_ref().#name)(#this_arg, #call_args)
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

enum ParsedType {
    Slice(Type),
    SliceMut(Type),
    Other(Type),
}

impl ToTokens for ParsedType {
    fn to_tokens(&self, toks: &mut proc_macro2::TokenStream) {
        match self {
            ParsedType::Slice(ty) | ParsedType::SliceMut(ty) | ParsedType::Other(ty) => {
                ty.to_tokens(toks)
            }
        }
    }
}

impl From<Type> for ParsedType {
    fn from(ty: Type) -> Self {
        match &ty {
            Type::Reference(ty) => match &*ty.elem {
                Type::Slice(slc) => {
                    if ty.mutability.is_none() {
                        return ParsedType::Slice(*slc.elem.clone());
                    } else {
                        return ParsedType::SliceMut(*slc.elem.clone());
                    }
                }
                _ => {}
            },
            _ => {}
        }

        ParsedType::Other(ty)
    }
}

enum ParsedReturnType {
    Nothing,
    Other(Type),
}

impl From<ReturnType> for ParsedReturnType {
    fn from(ty: ReturnType) -> Self {
        match ty {
            ReturnType::Default => ParsedReturnType::Nothing,
            ReturnType::Type(_, ty) => ParsedReturnType::Other((*ty).clone()),
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

impl ParsedType {
    fn from_receiver(ty: Receiver, typename: &str) -> Self {
        let mut s = String::new();

        if ty.reference.is_some() {
            s.push_str("&");
        }

        if ty.mutability.is_some() {
            s.push_str("mut ");
        }

        s.push_str(typename);

        let outty = Type::Verbatim(s.parse().unwrap());
        ParsedType::Other(outty)
    }
}

struct ParsedArg {
    name: Pat,
    ty: ParsedType,
}

impl From<FnArg> for ParsedArg {
    fn from(arg: FnArg) -> Self {
        match arg {
            FnArg::Receiver(ty) => ParsedArg::from_receiver(ty, "T"),
            FnArg::Typed(ty) => Self {
                name: *ty.pat,
                ty: (*ty.ty).into(),
            },
        }
    }
}

impl ParsedArg {
    fn from_receiver(ty: Receiver, typename: &str) -> Self {
        Self {
            name: Pat::Verbatim("this".parse().unwrap()),
            ty: ParsedType::from_receiver(ty, typename),
        }
    }
}

impl ToTokens for ParsedArg {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let ty = &self.ty;

        tokens.extend(quote! { #name: #ty, });
    }
}
