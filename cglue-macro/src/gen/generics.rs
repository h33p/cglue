use proc_macro2::TokenStream;
use quote::*;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::{Comma, Gt, Lt};
use syn::*;

#[derive(Clone)]
pub struct ParsedGenerics {
    /// Lifetime declarations on the left side of the type/trait.
    ///
    /// This may include any bounds it contains, for instance: `'a: 'b,`.
    pub life_declare: TokenStream,
    /// Declarations "using" the lifetimes i.e. has bounds stripped.
    ///
    /// For instance: `'a: 'b,` becomes just `'a,`.
    pub life_use: TokenStream,
    /// Type declarations on the left side of the type/trait.
    ///
    /// This may include any trait bounds it contains, for instance: `T: Clone,`.
    pub gen_declare: TokenStream,
    /// Declarations that "use" the traits i.e. has bounds stripped.
    ///
    /// For instance: `T: Clone,` becomes just `T,`.
    pub gen_use: TokenStream,
    /// Full `where Bounds` declaration.
    pub gen_where: TokenStream,
    /// All where predicates, without the `where` keyword.
    pub gen_where_bounds: TokenStream,
}

impl<'a> std::iter::FromIterator<&'a ParsedGenerics> for ParsedGenerics {
    fn from_iter<I: IntoIterator<Item = &'a ParsedGenerics>>(input: I) -> Self {
        let mut life_declare = TokenStream::new();
        let mut life_use = TokenStream::new();
        let mut gen_declare = TokenStream::new();
        let mut gen_use = TokenStream::new();
        let mut gen_where_bounds = TokenStream::new();

        for val in input {
            life_declare.extend(val.life_declare.clone());
            life_use.extend(val.life_use.clone());
            gen_declare.extend(val.gen_declare.clone());
            gen_use.extend(val.gen_use.clone());
            gen_where_bounds.extend(val.gen_where_bounds.clone());
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where: quote!(where #gen_where_bounds),
            gen_where_bounds,
        }
    }
}

impl From<Option<&Punctuated<GenericArgument, Comma>>> for ParsedGenerics {
    fn from(input: Option<&Punctuated<GenericArgument, Comma>>) -> Self {
        match input {
            Some(input) => Self::from(input),
            _ => Self {
                life_declare: quote!(),
                life_use: quote!(),
                gen_declare: quote!(),
                gen_use: quote!(),
                gen_where: quote!(),
                gen_where_bounds: quote!(),
            },
        }
    }
}

impl From<&Punctuated<GenericArgument, Comma>> for ParsedGenerics {
    fn from(input: &Punctuated<GenericArgument, Comma>) -> Self {
        let mut life = TokenStream::new();
        let mut gen = TokenStream::new();

        for param in input {
            match param {
                GenericArgument::Type(ty) => {
                    gen.extend(quote!(#ty, ));
                }
                GenericArgument::Const(_cn) => {
                    // TODO
                }
                GenericArgument::Lifetime(lifetime) => {
                    life.extend(quote!(#lifetime, ));
                }
                _ => {}
            }
        }

        Self {
            life_declare: life.clone(),
            life_use: life,
            gen_declare: gen.clone(),
            gen_use: gen,
            gen_where: quote!(),
            gen_where_bounds: quote!(),
        }
    }
}

impl From<&Generics> for ParsedGenerics {
    fn from(input: &Generics) -> Self {
        //let gen_declare = &input.params;
        let gen_where = &input.where_clause;
        let gen_where_bounds = gen_where.as_ref().map(|w| &w.predicates);

        let mut life_declare = TokenStream::new();
        let mut life_use = TokenStream::new();
        let mut gen_declare = TokenStream::new();
        let mut gen_use = TokenStream::new();

        for param in input.params.iter() {
            match param {
                GenericParam::Type(ty) => {
                    let ident = &ty.ident;
                    gen_use.extend(quote!(#ident, ));
                    gen_declare.extend(quote!(#ty, ));
                }
                GenericParam::Const(_cn) => {
                    // TODO
                }
                GenericParam::Lifetime(lt) => {
                    let lifetime = &lt.lifetime;
                    life_use.extend(quote!(#lifetime, ));
                    life_declare.extend(quote!(#lt, ));
                }
            }
        }

        let gen_where = match gen_where {
            Some(clause) => {
                if clause.predicates.trailing_punct() {
                    Some(quote!(#clause))
                } else {
                    Some(quote!(#clause,))
                }
            }
            _ => None,
        };

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where: quote!(#gen_where),
            gen_where_bounds: quote!(#gen_where_bounds),
        }
    }
}

impl Parse for ParsedGenerics {
    fn parse(input: ParseStream) -> Result<Self> {
        let gens = match input.parse::<Lt>() {
            Ok(_) => {
                let mut punct = Punctuated::new();

                while let Ok(arg) = input.parse::<GenericArgument>() {
                    punct.push_value(arg);

                    if let Ok(comma) = input.parse::<Comma>() {
                        punct.push_punct(comma);
                    } else {
                        break;
                    }
                }

                input.parse::<Gt>()?;
                Some(punct)
            }
            _ => None,
        };

        let ret = Self::from(gens.as_ref());

        if let Ok(mut clause) = input.parse::<WhereClause>() {
            if !clause.predicates.trailing_punct() {
                clause.predicates.push_punct(Default::default());
            }

            let predicates = &clause.predicates;
            Ok(Self {
                gen_where_bounds: quote!(#predicates),
                gen_where: quote!(#clause),
                ..ret
            })
        } else {
            Ok(ret)
        }
    }
}
