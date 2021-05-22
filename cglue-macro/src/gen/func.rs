use quote::ToTokens;
use std::string::ToString;
use syn::{Type, *};

const FN_PREFIX: &'static str = "cglue_wrapped_";

pub struct ParsedFunc {
    name: Ident,
    trait_name: String,
    safe: bool,
    abi: FuncAbi,
    receiver: Option<Receiver>,
    args: Vec<ParsedArg>,
    out: ParsedReturnType,
}

impl ParsedFunc {
    pub fn new(sig: Signature, trait_name: String) -> Self {
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

    pub fn vtbl_args(&self) -> String {
        let i1 = self
            .receiver.clone().map(ParsedArg::from)
            .into_iter()
            .map(|i| i.to_string());

        let i2 = self
            .args
            .iter()
            .map(ToString::to_string);

        i1.chain(i2)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn trait_args(&self) -> String {
        let i1 = self.receiver.as_ref().map(|i| i.to_token_stream().to_string())
            .into_iter();

        let i2 = self.args.iter().map(ToString::to_string);

        i1.chain(i2)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn chained_call_args(&self) -> String {
        self.args
            .iter()
            .map(|i| i.name.to_token_stream().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Create a VTable definition for this function
    pub fn vtbl_def(&self) -> String {
        format!(
            "{}: extern \"C\" fn({}) -> {},",
            self.name.to_string(),
            self.vtbl_args(),
            self.out.to_string()
        )
    }

    pub fn is_wrapped(&self) -> bool {
        self.abi == FuncAbi::Wrapped
    }

    /// Create a wrapper implementation body for this function
    ///
    /// If the function is ReprC already, it will not be wrapped and will return `None`
    pub fn cfunc_def(&self) -> Option<String> {
        if self.is_wrapped() {
            let name = self.name.to_string();

            let args = self.vtbl_args();
            let out = self.out.to_string();

            // TODO: add support for writing Ok result to MaybeUninit
            let mut out = format!(
                "extern \"C\" fn {}{}<T: {}>({}) -> {} {{",
                FN_PREFIX, name, self.trait_name, args, out
            );

            out.push_str(&format!("let ret = this.{}({});", name, ""));

            // TODO: add checks for result wrapping
            out.push_str("ret");

            out.push_str("}");

            Some(out)
        } else {
            None
        }
    }

    pub fn vtbl_default_def(&self) -> String {
        let name = self.name.to_string();

        if self.is_wrapped() {
            format!("{}: {}{},", name, FN_PREFIX, name)
        } else {
            format!("{}: T::{},", name, name)
        }
    }

    pub fn trait_impl(&self) -> String {
        let name = self.name.to_string();
        let args = self.trait_args();
        let out = self.out.to_string();
        let call_args = self.chained_call_args();

        let this_arg = match &self.receiver {
            Some(x) => if x.mutability.is_some() {
                "self.cobj_mut()"
            } else {
                "self.cobj_ref()"
            },
            _ => "()"
        };

        format!(
            r#"
                #[inline(always)]
                {} fn {} ({}) -> {} {{
                    (self.as_ref().{})({}, {})
                }}
            "#, self.abi.prefix(), name, args, out, name, this_arg, call_args
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
enum FuncAbi {
    ReprC,
    Wrapped,
}

impl FuncAbi {
    pub fn prefix(&self) -> &'static str {
        match self {
            FuncAbi::ReprC => "extern \"C\"",
            FuncAbi::Wrapped => ""
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

impl ToString for ParsedReturnType {
    fn to_string(&self) -> String {
        match &self {
            ParsedReturnType::Nothing => "()".to_string(),
            ParsedReturnType::Other(x) => x.to_token_stream().to_string(),
        }
    }
}

impl From<Receiver> for ParsedType {
    fn from(ty: Receiver) -> Self {
        let mut s = String::new();

        if ty.reference.is_some() {
            s.push_str("&");
        }

        if ty.mutability.is_some() {
            s.push_str("mut ");
        }

        s.push_str("T");

        let outty = Type::Verbatim(s.parse().unwrap());
        ParsedType::Other(outty)
    }
}

struct ParsedArg {
    name: Pat,
    ty: ParsedType,
}

impl ToString for ParsedArg {
    fn to_string(&self) -> String {
        format!(
            "{}: {}",
            self.name.to_token_stream().to_string(),
            self.ty.to_token_stream().to_string()
        )
    }
}

impl From<FnArg> for ParsedArg {
    fn from(arg: FnArg) -> Self {
        match arg {
            FnArg::Receiver(ty) => ty.into(),
            FnArg::Typed(ty) => Self {
                name: *ty.pat,
                ty: (*ty.ty).into(),
            },
        }
    }
}

impl From<Receiver> for ParsedArg {
    fn from(ty: Receiver) -> Self {
        Self {
            name: Pat::Verbatim("this".parse().unwrap()),
            ty: ty.into(),
        }
    }
}
