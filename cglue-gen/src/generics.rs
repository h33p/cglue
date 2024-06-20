use crate::util::{parse_punctuated, recurse_type_to_path};
use proc_macro2::TokenStream;
use quote::*;
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::{Comma, Gt, Lt};
use syn::*;

fn ident_path(ident: Ident) -> Type {
    let mut path = Path {
        leading_colon: None,
        segments: Punctuated::new(),
    };

    path.segments.push_value(PathSegment {
        ident,
        arguments: Default::default(),
    });

    Type::Path(TypePath { qself: None, path })
}

fn ty_ident(ty: &Type) -> Option<&Ident> {
    if let Type::Path(path) = ty {
        if path.qself.is_none() {
            path.path.get_ident()
        } else {
            None
        }
    } else {
        None
    }
}

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
    pub gen_use: Punctuated<Type, Comma>,
    /// All where predicates, without the `where` keyword.
    pub gen_where_bounds: Punctuated<WherePredicate, Comma>,
    /// Remap generic T to a particular type using T = type syntax.
    ///
    /// Then, when generics get cross referenced, all concrete T declarations get removed, and T
    /// uses get replaced with concrete types.
    pub gen_remaps: HashMap<Ident, Type>,
}

impl ParsedGenerics {
    pub fn declare_without_nonstatic_bounds(&self) -> Punctuated<TypeParam, Comma> {
        let mut ret = self.gen_declare.clone();

        for p in ret.iter_mut() {
            p.bounds = std::mem::take(&mut p.bounds)
                .into_iter()
                .filter(|b| {
                    if let TypeParamBound::Lifetime(lt) = b {
                        lt.ident == "static"
                    } else {
                        true
                    }
                })
                .collect();
        }

        ret
    }

    #[cfg(feature = "layout_checks")]
    pub fn declare_lt_for_all(&self, lt: &TokenStream) -> TokenStream {
        let mut ts = TokenStream::new();

        for p in &self.gen_use {
            ts.extend(quote!(#p: #lt,));
        }

        ts
    }

    #[cfg(not(feature = "layout_checks"))]
    pub fn declare_lt_for_all(&self, _: &TokenStream) -> TokenStream {
        Default::default()
    }

    pub fn declare_sabi_for_all(&self, crate_path: &TokenStream) -> TokenStream {
        let mut ts = TokenStream::new();

        for p in &self.gen_use {
            ts.extend(quote!(#p: #crate_path::trait_group::GenericTypeBounds,));
        }

        ts
    }

    /// This function cross references input lifetimes and returns a new Self
    /// that only contains generic type information about those types.
    pub fn cross_ref<'a>(&self, input: impl IntoIterator<Item = &'a ParsedGenerics>) -> Self {
        let mut applied_lifetimes = HashSet::<&Ident>::new();
        let mut applied_typenames = HashSet::<&Type>::new();

        let mut life_declare = Punctuated::new();
        let mut life_use = Punctuated::new();
        let mut gen_declare = Punctuated::new();
        let mut gen_use = Punctuated::<Type, _>::new();
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
                    .expect("Gen 1");

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
                    .expect("Gen 2");

                gen_declare.push_value(decl.clone());
                gen_declare.push_punct(Default::default());
                gen_use.push_value(ident_path(decl.ident.clone()));
                gen_use.push_punct(Default::default());

                applied_typenames.insert(ident);
            }
        }

        for wb in self.gen_where_bounds.iter() {
            if match wb {
                WherePredicate::Type(ty) => applied_typenames.contains(&ty.bounded_ty),
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
            gen_remaps: Default::default(),
        }
    }

    pub fn merge_remaps(&mut self, other: &mut ParsedGenerics) {
        self.gen_remaps
            .extend(std::mem::take(&mut other.gen_remaps));
        other.gen_remaps = self.gen_remaps.clone();
    }

    pub fn merge_and_remap(&mut self, other: &mut ParsedGenerics) {
        self.merge_remaps(other);
        self.remap_types();
        other.remap_types();
    }

    pub fn remap_types(&mut self) {
        let old_gen_declare = std::mem::take(&mut self.gen_declare);
        let old_gen_use = std::mem::take(&mut self.gen_use);

        for val in old_gen_declare.into_pairs() {
            match val {
                punctuated::Pair::Punctuated(p, punc) => {
                    if !self.gen_remaps.contains_key(&p.ident) {
                        self.gen_declare.push_value(p);
                        self.gen_declare.push_punct(punc);
                    }
                }
                punctuated::Pair::End(p) => {
                    if !self.gen_remaps.contains_key(&p.ident) {
                        self.gen_declare.push_value(p);
                    }
                }
            }
        }

        for val in old_gen_use.into_pairs() {
            match val {
                punctuated::Pair::Punctuated(p, punc) => {
                    if let Some(ident) = ty_ident(&p) {
                        self.gen_use
                            .push_value(self.gen_remaps.get(ident).cloned().unwrap_or(p));
                    } else {
                        self.gen_use.push_value(p);
                    }
                    self.gen_use.push_punct(punc);
                }
                punctuated::Pair::End(p) => {
                    if let Some(ident) = ty_ident(&p) {
                        self.gen_use
                            .push_value(self.gen_remaps.get(ident).cloned().unwrap_or(p));
                    } else {
                        self.gen_use.push_value(p);
                    }
                }
            }
        }
    }

    /// Generate phantom data definitions for all lifetimes and types used.
    pub fn phantom_data_definitions(&self) -> TokenStream {
        let mut stream = TokenStream::new();

        for ty in self.gen_declare.iter() {
            let ty_ident = format_ident!("_ty_{}", ty.ident.to_string().to_lowercase());
            let ty = &ty.ident;
            stream.extend(quote!(#ty_ident: ::core::marker::PhantomData<#ty>,));
        }

        stream
    }

    /// Generate phantom data initializations for all lifetimes and types used.
    pub fn phantom_data_init(&self) -> TokenStream {
        let mut stream = TokenStream::new();

        for ty in self.gen_declare.iter() {
            let ty_ident = format_ident!("_ty_{}", ty.ident.to_string().to_lowercase());
            stream.extend(quote!(#ty_ident: ::core::marker::PhantomData{},));
        }

        stream
    }

    /// Replace generic arguments on the type with ones stored within Self.
    ///
    /// The same generic args are replaced as the ones extracted from `util::recurse_type_to_path`.
    pub fn replace_on_type(&self, ty: &mut Type) {
        recurse_type_to_path(ty, |path| {
            let mut generics = None;
            for part in path.segments.pairs_mut() {
                if let punctuated::Pair::End(p) = part {
                    if let PathArguments::AngleBracketed(arg) = &mut p.arguments {
                        generics = Some(arg);
                    }
                }
            }

            let life_use = &self.life_use;
            let gen_use = &self.gen_use;

            if let Some(generics) = generics {
                *generics = syn::parse2(quote!(<#life_use #gen_use>)).unwrap();
            }

            Some(())
        });
    }

    pub fn extract_lifetimes(&mut self, ty: &Type) {
        fn extract_nonpath_lifetimes(ty: &Type, out: &mut HashSet<Lifetime>) {
            match ty {
                Type::Array(TypeArray { elem, .. }) => extract_nonpath_lifetimes(elem, out),
                Type::Group(TypeGroup { elem, .. }) => extract_nonpath_lifetimes(elem, out),
                Type::Paren(TypeParen { elem, .. }) => extract_nonpath_lifetimes(elem, out),
                Type::Ptr(TypePtr { elem, .. }) => extract_nonpath_lifetimes(elem, out),
                Type::Reference(TypeReference { elem, lifetime, .. }) => {
                    if let Some(lifetime) = lifetime {
                        out.insert(lifetime.clone());
                    }
                    extract_nonpath_lifetimes(elem, out)
                }
                Type::Slice(TypeSlice { elem, .. }) => extract_nonpath_lifetimes(elem, out),
                _ => (),
            }
        }

        let mut lifetimes = HashSet::new();
        extract_nonpath_lifetimes(ty, &mut lifetimes);

        let existing_lifetimes = self
            .life_declare
            .iter()
            .map(|l| &l.lifetime)
            .collect::<HashSet<&Lifetime>>();

        for lt in existing_lifetimes {
            lifetimes.remove(lt);
        }

        for lt in lifetimes {
            self.life_use.push_value(lt.clone());
            self.life_use.push_punct(Default::default());
            self.life_declare.push_value(LifetimeDef::new(lt));
            self.life_declare.push_punct(Default::default());
        }
    }
}

impl<'a> std::iter::FromIterator<&'a ParsedGenerics> for ParsedGenerics {
    fn from_iter<I: IntoIterator<Item = &'a ParsedGenerics>>(input: I) -> Self {
        let mut life_declare = Punctuated::new();
        let mut life_declared = HashSet::<&Ident>::new();

        let mut life_use = Punctuated::new();
        let mut gen_use = Punctuated::new();

        let mut gen_declare = Punctuated::new();
        let mut gen_declared = HashSet::<&Ident>::new();

        let mut gen_where_bounds = Punctuated::new();

        let mut gen_remaps = HashMap::default();

        for val in input {
            life_use.extend(val.life_use.clone());
            gen_use.extend(val.gen_use.clone());

            for life in val.life_declare.pairs() {
                let (val, punct) = life.into_tuple();
                if life_declared.contains(&val.lifetime.ident) {
                    continue;
                }
                life_declare.push_value(val.clone());
                if let Some(punct) = punct {
                    life_declare.push_punct(*punct);
                }
                life_declared.insert(&val.lifetime.ident);
            }

            for gen in val.gen_declare.pairs() {
                let (val, punct) = gen.into_tuple();
                if gen_declared.contains(&val.ident) {
                    continue;
                }
                gen_declare.push_value(val.clone());
                if let Some(punct) = punct {
                    gen_declare.push_punct(*punct);
                }
                gen_declared.insert(&val.ident);
            }

            gen_where_bounds.extend(val.gen_where_bounds.clone());
            gen_remaps.extend(val.gen_remaps.clone());
        }

        if !gen_where_bounds.empty_or_trailing() {
            gen_where_bounds.push_punct(Default::default());
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds,
            gen_remaps,
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
                gen_remaps: Default::default(),
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
        let mut gen_remaps = HashMap::new();

        for param in input {
            match param {
                GenericArgument::Type(ty) => {
                    if let Some(ident) = ty_ident(ty).cloned() {
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
                    gen_use.push_value(ty.clone());
                    gen_use.push_punct(Default::default());
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
                GenericArgument::Constraint(constraint) => {
                    gen_use.push_value(ident_path(constraint.ident.clone()));
                    gen_use.push_punct(Default::default());
                    gen_declare.push_value(TypeParam {
                        attrs: vec![],
                        ident: constraint.ident.clone(),
                        colon_token: None,
                        bounds: constraint.bounds.clone(),
                        eq_token: None,
                        default: None,
                    });
                    gen_declare.push_punct(Default::default());
                }
                GenericArgument::Binding(bind) => {
                    gen_use.push_value(bind.ty.clone());
                    gen_use.push_punct(Default::default());
                    gen_remaps.insert(bind.ident.clone(), bind.ty.clone());
                }
            }
        }

        Self {
            life_declare,
            life_use,
            gen_declare,
            gen_use,
            gen_where_bounds: Punctuated::new(),
            gen_remaps,
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
                    gen_use.push_value(ident_path(ty.ident.clone()));
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
            gen_remaps: Default::default(),
        }
    }
}

fn parse_generic_arguments(input: ParseStream) -> Punctuated<GenericArgument, Comma> {
    parse_punctuated(input)
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
    pub expr: Box<Expr>,
    pub target: GenericType,
    pub ident: TokenStream,
}

impl Parse for GenericCastType {
    fn parse(input: ParseStream) -> Result<Self> {
        let cast: ExprCast = input.parse()?;

        let expr = cast.expr;
        let target = GenericType::from_type(&cast.ty, true);
        let ident = GenericType::from_type(&cast.ty, false).target;

        Ok(Self {
            expr,
            target,
            ident,
        })
    }
}

pub struct GroupCastType {
    pub expr: Box<Expr>,
    pub target: GenericType,
    pub ident: TokenStream,
}

impl Parse for GroupCastType {
    fn parse(input: ParseStream) -> Result<Self> {
        let cast: ExprCast = input.parse()?;

        let expr = cast.expr;
        let target = GenericType::from_type(&cast.ty, true);
        let ident = GenericType::from_type(&cast.ty, false).target;

        Ok(Self {
            expr,
            target,
            ident,
        })
    }
}

#[derive(Clone)]
pub struct GenericType {
    /// Path to type (core:: in core::Option<T>)
    pub path: Path,
    /// Separator to use, this depends on `cast_to_group` parameter
    pub gen_separator: TokenStream,
    /// Generic lifetime parameters (there isn't an example in core::Option<T>)
    pub generic_lifetimes: Punctuated<Lifetime, Comma>,
    /// Generic type parameters (T in core::Option<T>)
    pub generic_types: Punctuated<Type, Comma>,
    /// The resulting type (Option in core::Option<T>)
    pub target: TokenStream,
}

impl GenericType {
    pub fn push_lifetime_start(&mut self, lifetime: &Lifetime) {
        self.generic_lifetimes.insert(0, lifetime.clone());
        if !self.generic_lifetimes.trailing_punct() {
            self.generic_lifetimes.push_punct(Default::default());
        }
    }

    pub fn push_types_start(&mut self, types: TokenStream) {
        let typestr = types.to_string();
        let mut types =
            syn::parse::Parser::parse2(Punctuated::<Type, Comma>::parse_terminated, types)
                .expect(&format!("Invalid types provided: {}", typestr));

        if !types.trailing_punct() {
            types.push_punct(Default::default());
        }

        // Swap here, because types becomes the start
        std::mem::swap(&mut self.generic_types, &mut types);

        self.generic_types.extend(types.into_iter());

        if !self.generic_types.trailing_punct() {
            self.generic_types.push_punct(Default::default());
        }
    }

    pub fn push_types_end(&mut self, types: TokenStream) {
        let typestr = types.to_string();
        let types = syn::parse::Parser::parse2(Punctuated::<Type, Comma>::parse_terminated, types)
            .expect(&format!("Invalid types provided: {}", typestr));

        // Unlike in push_types_start, we do not swap them

        if !self.generic_types.trailing_punct() {
            self.generic_types.push_punct(Default::default());
        }

        self.generic_types.extend(types.into_iter());

        if !self.generic_types.trailing_punct() {
            self.generic_types.push_punct(Default::default());
        }
    }

    fn from_type(target: &Type, cast_to_obj: bool) -> Self {
        let (path, mut target, generics) = match target {
            Type::Path(ty) => {
                let (path, target, generics) =
                    crate::util::split_path_ident(&ty.path).expect("Gen 3");
                (path, quote!(#target), generics)
            }
            x => (
                Path {
                    leading_colon: None,
                    segments: Default::default(),
                },
                quote!(#x),
                None,
            ),
        };

        let (generic_lifetimes, mut generic_types) = match &generics {
            Some(params) => {
                let pg = ParsedGenerics::from(params);
                (pg.life_use, pg.gen_use)
            }
            _ => Default::default(),
        };

        let gen_separator = if cast_to_obj {
            if generics.is_some() {
                let infer = Type::Infer(TypeInfer {
                    underscore_token: Default::default(),
                });
                generic_types.insert(0, infer.clone());
                generic_types.insert(0, infer);

                if !generic_types.trailing_punct() {
                    generic_types.push_punct(Default::default());
                }
            }
            target = format_ident!("{}Base", target.to_string()).to_token_stream();

            quote!(::)
        } else {
            quote!()
        };

        Self {
            path,
            gen_separator,
            generic_lifetimes,
            generic_types,
            target,
        }
    }
}

impl ToTokens for GenericType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.path.to_token_stream());
        tokens.extend(self.target.clone());
        let generic_lifetimes = &self.generic_lifetimes;
        let generic_types = &self.generic_types;
        if !generic_lifetimes.is_empty() || !generic_types.is_empty() {
            tokens.extend(self.gen_separator.clone());
            tokens.extend(quote!(<#generic_lifetimes #generic_types>));
        }
    }
}

impl Parse for GenericType {
    fn parse(input: ParseStream) -> Result<Self> {
        let target: Type = input.parse()?;
        Ok(Self::from_type(&target, false))
    }
}
