use proc_macro2::TokenStream;
use quote::*;
use std::collections::HashSet;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::{Comma, Gt, Lt};
use syn::*;

#[derive(Clone)]
pub struct ParsedGenerics {
    /// Lifetime declarations on the left side of the type/trait.
    ///
    /// This may include any bounds it contains, for instance: `'a: 'b,`.
    pub life_declare: Punctuated<LifetimeDef, Comma>,
    /// Declarations "using" the lifetimes i.e. has bounds stripped.
    ///
    /// For instance: `'a: 'b,` becomes just `'a,`.
    pub life_use: Punctuated<Lifetime, Comma>,
    /// Type declarations on the left side of the type/trait.
    ///
    /// This may include any trait bounds it contains, for instance: `T: Clone,`.
    pub gen_declare: Punctuated<TypeParam, Comma>,
    /// Declarations that "use" the traits i.e. has bounds stripped.
    ///
    /// For instance: `T: Clone,` becomes just `T,`.
    pub gen_use: Punctuated<Ident, Comma>,
    /// All where predicates, without the `where` keyword.
    pub gen_where_bounds: Punctuated<WherePredicate, Comma>,
}

impl ParsedGenerics {
    /// This function cross references input lifetimes and returns a new Self
    /// that only contains generic type information about those types.
    pub fn cross_ref<'a>(&self, input: impl IntoIterator<Item = &'a ParsedGenerics>) -> Self {
        let mut applied_lifetimes = HashSet::<&Ident>::new();
        let mut applied_typenames = HashSet::<&Ident>::new();

        let mut life_declare = Punctuated::new();
        let mut life_use = Punctuated::new();
        let mut gen_declare = Punctuated::new();
        let mut gen_use = Punctuated::new();
        let mut gen_where_bounds = Punctuated::new();

        for ParsedGenerics {
            life_use: in_lu,
            gen_use: in_gu,
            ..
        } in input
        {
            for lt in in_lu.iter() {
                if applied_lifetimes.contains(&lt.ident) {
                    continue;
                }

                let decl = self
                    .life_declare
                    .iter()
                    .find(|ld| ld.lifetime.ident == lt.ident)
                    .unwrap();

                life_declare.push_value(decl.clone());
                life_declare.push_punct(Default::default());
                life_use.push_value(decl.lifetime.clone());
                life_use.push_punct(Default::default());

                applied_lifetimes.insert(&lt.ident);
            }

            for ty in in_gu.iter() {
                if applied_typenames.contains(&ty) {
                    continue;
                }

                let (decl, ident) = self
                    .gen_declare
                    .iter()
                    .zip(self.gen_use.iter())
                    .find(|(_, ident)| *ident == ty)
                    .unwrap();

                gen_declare.push_value(decl.clone());
                gen_declare.push_punct(Default::default());
                gen_use.push_value(decl.ident.clone());
                gen_use.push_punct(Default::default());

                applied_typenames.insert(&ident);
            }
        }

        for wb in self.gen_where_bounds.iter() {
            if match wb {
                WherePredicate::Type(ty) => {
                    if let Ok(ident) = parse2::<Ident>(ty.bounded_ty.to_token_stream()) {
                        applied_typenames.contains(&ident)
                    } else {
                        // TODO: What to do with other bounds?
                        false
                    }
                }
                WherePredicate::Lifetime(lt) => applied_lifetimes.contains(&lt.lifetime.ident),
                _ => false,
            } {
                gen_where_bounds.push_value(wb.clone());
                gen_where_bounds.push_punct(Default::default());
            }
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds,
        }
    }
}

impl<'a> std::iter::FromIterator<&'a ParsedGenerics> for ParsedGenerics {
    fn from_iter<I: IntoIterator<Item = &'a ParsedGenerics>>(input: I) -> Self {
        let mut life_declare = Punctuated::new();
        let mut life_use = Punctuated::new();
        let mut gen_declare = Punctuated::new();
        let mut gen_use = Punctuated::new();
        let mut gen_where_bounds = Punctuated::new();

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
            gen_where_bounds,
        }
    }
}

impl From<Option<&Punctuated<GenericArgument, Comma>>> for ParsedGenerics {
    fn from(input: Option<&Punctuated<GenericArgument, Comma>>) -> Self {
        match input {
            Some(input) => Self::from(input),
            _ => Self {
                life_declare: Punctuated::new(),
                life_use: Punctuated::new(),
                gen_declare: Punctuated::new(),
                gen_use: Punctuated::new(),
                gen_where_bounds: Punctuated::new(),
            },
        }
    }
}

impl From<&Punctuated<GenericArgument, Comma>> for ParsedGenerics {
    fn from(input: &Punctuated<GenericArgument, Comma>) -> Self {
        let mut life_declare = Punctuated::new();
        let mut life_use = Punctuated::new();
        let mut gen_declare = Punctuated::new();
        let mut gen_use = Punctuated::new();

        for param in input {
            match param {
                GenericArgument::Type(ty) => {
                    let ident = format_ident!("{}", ty.to_token_stream().to_string());
                    gen_use.push_value(ident.clone());
                    gen_use.push_punct(Default::default());
                    gen_declare.push_value(TypeParam {
                        attrs: vec![],
                        ident,
                        colon_token: None,
                        bounds: Punctuated::new(),
                        eq_token: None,
                        default: None,
                    });
                    gen_declare.push_punct(Default::default());
                }
                GenericArgument::Const(_cn) => {
                    // TODO
                }
                GenericArgument::Lifetime(lifetime) => {
                    life_use.push_value(lifetime.clone());
                    life_use.push_punct(Default::default());
                    life_declare.push_value(LifetimeDef {
                        attrs: vec![],
                        lifetime: lifetime.clone(),
                        colon_token: None,
                        bounds: Punctuated::new(),
                    });
                    life_declare.push_punct(Default::default());
                }
                _ => {}
            }
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds: Punctuated::new(),
        }
    }
}

impl From<&Generics> for ParsedGenerics {
    fn from(input: &Generics) -> Self {
        let gen_where = &input.where_clause;
        let gen_where_bounds = gen_where.as_ref().map(|w| &w.predicates);

        let mut life_declare = Punctuated::new();
        let mut life_use = Punctuated::new();
        let mut gen_declare = Punctuated::new();
        let mut gen_use = Punctuated::new();

        for param in input.params.iter() {
            match param {
                GenericParam::Type(ty) => {
                    gen_use.push_value(ty.ident.clone());
                    gen_use.push_punct(Default::default());
                    gen_declare.push_value(ty.clone());
                    gen_declare.push_punct(Default::default());
                }
                GenericParam::Const(_cn) => {
                    // TODO
                }
                GenericParam::Lifetime(lt) => {
                    let lifetime = &lt.lifetime;
                    life_use.push_value(lifetime.clone());
                    life_use.push_punct(Default::default());
                    life_declare.push_value(lt.clone());
                    life_declare.push_punct(Default::default());
                }
            }
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds: gen_where_bounds.cloned().unwrap_or_else(Punctuated::new),
        }
    }
}

fn parse_generic_arguments(input: ParseStream) -> Punctuated<GenericArgument, Comma> {
    let mut punct = Punctuated::new();

    while let Ok(arg) = input.parse::<GenericArgument>() {
        punct.push_value(arg);

        if let Ok(comma) = input.parse::<Comma>() {
            punct.push_punct(comma);
        } else {
            break;
        }
    }

    punct
}

impl Parse for ParsedGenerics {
    fn parse(input: ParseStream) -> Result<Self> {
        let gens = match input.parse::<Lt>() {
            Ok(_) => {
                let punct = parse_generic_arguments(input);
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
                gen_where_bounds: predicates.clone(),
                ..ret
            })
        } else {
            Ok(ret)
        }
    }
}

pub struct GenericCastType {
    pub ident: Box<Expr>,
    pub target: GenericType,
}

impl Parse for GenericCastType {
    fn parse(input: ParseStream) -> Result<Self> {
        let cast: ExprCast = input.parse()?;

        let ident = cast.expr;
        let target = GenericType::from_type(&*cast.ty, true)?;

        Ok(Self { ident, target })
    }
}

#[derive(Clone)]
pub struct GenericType {
    pub path: TokenStream,
    pub gen_separator: TokenStream,
    pub generics: TokenStream,
    pub target: TokenStream,
}

impl GenericType {
    pub fn push_lifetime_start(&mut self, lifetime: &Lifetime) {
        let gen = &self.generics;
        self.generics = quote!(#lifetime, #gen);
    }

    pub fn push_types_start(&mut self, types: TokenStream) {
        let generics = std::mem::replace(&mut self.generics, TokenStream::new());

        let ParsedGenerics {
            life_declare,
            gen_declare,
            ..
        } = parse2::<ParsedGenerics>(quote!(<#generics>)).unwrap();

        self.generics
            .extend(quote!(#life_declare #types #gen_declare));
    }

    fn from_type(target: &Type, cast_to_group: bool) -> Result<Self> {
        let (path, target, generics) = match target {
            Type::Path(ty) => {
                let (path, target, generics) = crate::util::split_path_ident(&ty.path).unwrap();
                (quote!(#path), quote!(#target), generics)
            }
            x => (quote!(), quote!(#x), None),
        };

        let (gen_separator, generics) = match (cast_to_group, generics) {
            (true, Some(params)) => {
                let pg = ParsedGenerics::from(&params);

                let life = &pg.life_use;
                let gen = &pg.gen_use;

                (quote!(::), quote!(#life _, _, #gen))
            }
            (false, Some(params)) => (quote!(), quote!(#params)),
            (true, _) => (quote!(::), quote!()),
            _ => (quote!(), quote!()),
        };

        Ok(Self {
            path,
            gen_separator,
            generics,
            target,
        })
    }
}

impl ToTokens for GenericType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.path.clone());
        tokens.extend(self.target.clone());
        let generics = &self.generics;
        if !generics.is_empty() {
            tokens.extend(self.gen_separator.clone());
            tokens.extend(quote!(<#generics>));
        }
    }
}

impl Parse for GenericType {
    fn parse(input: ParseStream) -> Result<Self> {
        let target: Type = input.parse()?;
        Self::from_type(&target, false)
    }
}
