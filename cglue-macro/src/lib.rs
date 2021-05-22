extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use std::string::ToString;
use syn::{punctuated::*, token::*, Type, *};

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
            FnArg::Receiver(ty) => Self {
                name: Pat::Verbatim("this".parse().unwrap()),
                ty: ty.into(),
            },
            FnArg::Typed(ty) => Self {
                name: *ty.pat,
                ty: (*ty.ty).into(),
            },
        }
    }
}

struct ParsedFunc {
    name: Ident,
    trait_name: String,
    safe: bool,
    abi: FuncAbi,
    args: Vec<ParsedArg>,
    out: ParsedReturnType,
}

const FN_PREFIX: &'static str = "cglue_wrapped_";

impl ParsedFunc {
    fn new(sig: Signature, trait_name: String) -> Self {
        let name = sig.ident;
        let safe = sig.unsafety.is_none();
        let abi = From::from(sig.abi);
        let mut args: Vec<ParsedArg> = vec![];

        for input in sig.inputs.into_iter() {
            args.push(input.into());
        }

        let out = From::from(sig.output);

        println!("{} {} {:?}", name, safe, abi);

        for a in &args {
            let name = &a.name;
            let ty = &a.ty;
            println!("{} {}", quote!(#name).to_string(), quote!(#ty).to_string());
        }

        Self {
            name,
            trait_name,
            safe,
            abi,
            args,
            out,
        }
    }

    fn vtbl_args(&self) -> String {
        self.args
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Create a VTable definition for this function
    fn vtbl_def(&self) -> String {
        format!(
            "{}: extern \"C\" fn({}) -> {},",
            self.name.to_string(),
            self.vtbl_args(),
            self.out.to_string()
        )
    }

    fn is_wrapped(&self) -> bool {
        self.abi == FuncAbi::Wrapped
    }

    /// Create a wrapper implementation body for this function
    ///
    /// If the function is ReprC already, it will not be wrapped and will return `None`
    fn cfunc_def(&self) -> Option<String> {
        if self.is_wrapped() {
            let name = self.name.to_string();

            let args = self.vtbl_args();
            let out = self.out.to_string();

            let mut out = format!(
                "extern \"C\" fn {}{}<T: {}>({}) -> {} {{",
                FN_PREFIX, name, self.trait_name, args, out
            );

            out.push_str(&format!("let ret = this.{}({});", name, ""));

            out.push_str("ret");

            out.push_str("}");

            Some(out)
        } else {
            None
        }
    }

    fn vtbl_default_def(&self) -> String {
        let name = self.name.to_string();

        if self.is_wrapped() {
            format!("{}: {}{},", name, FN_PREFIX, name)
        } else {
            format!("{}: T::{},", name, name)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum FuncAbi {
    ReprC,
    Wrapped,
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

#[proc_macro]
pub fn cglue_trait_group(args: TokenStream) -> TokenStream {
    let gen = quote! {};

    gen.into()
}

#[proc_macro_attribute]
pub fn cglue_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);

    let trname = tr.ident.to_string();

    let name: proc_macro2::TokenStream = format!("CGlueVtbl{}<T: {}>", trname, trname)
        .parse()
        .unwrap();

    let mut funcs = vec![];

    for item in &tr.items {
        if let TraitItem::Method(m) = item {
            funcs.push(ParsedFunc::new(m.sig.clone(), trname.clone()));
        }
    }

    println!("name: {}", name.to_string());

    // Define the vtable
    let mut vtbl = format!(
        r#"
        /// CGlue vtable for trait {}.
        ///
        /// This virtual function table contains ABI-safe interface for the given trait.
        #[repr(C)]
        struct CGlueVtbl{}<T> {{
    "#,
        trname, trname
    );

    for func in &funcs {
        vtbl.push_str(&func.vtbl_def());
    }

    vtbl.push_str("}");

    println!("{}", vtbl);

    let parsed_vtbl: proc_macro2::TokenStream = vtbl.parse().unwrap();

    // Define the default implementation for the vtable reference
    let mut vtbl_default = format!(
        r#"
    impl<'a, T: {}> Default for &'a CGlueVtbl{}<T> {{
        /// Create a static vtable for the given type.
        fn default() -> Self {{
            &CGlueVtbl{} {{
    "#,
        trname, trname, trname
    );

    for func in &funcs {
        vtbl_default.push_str(&func.vtbl_default_def());
    }

    vtbl_default.push_str(
        r#"
            }
        }
    }"#,
    );

    let parsed_vtbl_default: proc_macro2::TokenStream = vtbl_default.parse().unwrap();

    // Define wrapped functions for the vtable
    let mut cfuncs = String::new();

    for func in funcs.iter().filter_map(ParsedFunc::cfunc_def) {
        cfuncs.push_str(&func);
    }

    let parsed_cfuncs: proc_macro2::TokenStream = cfuncs.parse().unwrap();

    // Define safe opaque conversion for the vtable
    let vtbl_opaque = format!(
        r#"
        /// Opaque type for trait {} vtable, used in trait groups.
        pub type OpaqueCGlueVtbl{} = CGlueVtbl{}<core::ffi::c_void>;

        impl<T: {}> CGlueVtbl{}<T> {{
            /// Convert a concrete vtable to an opaque one.
            pub fn as_opaque(&self) -> &OpaqueCGlueVtbl{} {{
                unsafe {{ &*(self as *const Self as *const OpaqueCGlueVtbl{}) }}
            }}
        }}
    "#,
        trname, trname, trname, trname, trname, trname, trname
    );

    let parsed_vtbl_opaque: proc_macro2::TokenStream = vtbl_opaque.parse().unwrap();

    // Glue it all together
    let gen = quote! {
        #tr
        #parsed_vtbl
        #parsed_vtbl_default
        #parsed_cfuncs
        #parsed_vtbl_opaque
    };

    eprintln!("{}", gen);

    gen.into()
}
