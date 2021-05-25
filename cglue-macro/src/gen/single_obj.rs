use syn::parse::{Parse, ParseStream};
use syn::*;

pub struct ObjStruct {
    pub ident: Ident,
    pub target: Ident,
}

impl Parse for ObjStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let _as_token: Token![as] = input.parse()?;
        let target: Ident = input.parse()?;

        Ok(ObjStruct { ident, target })
    }
}
