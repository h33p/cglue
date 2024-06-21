use super::ext::*;
use super::generics::ParsedGenerics;
use crate::util::*;
use itertools::*;
use proc_macro2::TokenStream;
use quote::*;
use std::collections::{BTreeMap, HashMap};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::*;

pub struct AliasPath {
    path: Path,
    alias: Option<Ident>,
}

impl Parse for AliasPath {
    fn parse(input: ParseStream) -> Result<Self> {
        let path = input.parse()?;

        let alias = if input.parse::<Token![=]>().is_ok() {
            Some(input.parse::<Ident>()?)
        } else {
            None
        };

        Ok(Self { path, alias })
    }
}

impl AliasPath {
    fn prelude_remap(self) -> Self {
        Self {
            path: prelude_remap(self.path),
            alias: self.alias,
        }
    }

    fn ext_abs_remap(self) -> Self {
        Self {
            path: ext_abs_remap(self.path),
            alias: self.alias,
        }
    }
}

/// Describes information about a single trait.
pub struct TraitInfo {
    path: Path,
    raw_ident: Ident,
    name_ident: Ident,
    generics: ParsedGenerics,
    assocs: ParsedGenerics,
    vtbl_name: Ident,
    ret_tmp_typename: Ident,
    ret_tmp_name: Ident,
    enable_vtbl_name: Ident,
    lc_name: Ident,
    vtbl_typename: Ident,
    vtbl_get_ident: Ident,
    assoc_bind_ident: Ident,
}

impl PartialEq for TraitInfo {
    fn eq(&self, o: &Self) -> bool {
        self.name_ident == o.name_ident
    }
}

impl Eq for TraitInfo {}

impl Ord for TraitInfo {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.name_ident.cmp(&o.name_ident)
    }
}

impl PartialOrd for TraitInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<AliasPath> for TraitInfo {
    fn from(in_path: AliasPath) -> Self {
        let (path, raw_ident, mut gens) =
            split_path_ident(&in_path.path).expect("Failed to split path by idents");

        let mut name_ident = raw_ident.clone();

        let mut lc_ident = raw_ident.to_string().to_lowercase();

        if let Some(alias) = in_path.alias {
            lc_ident = alias.to_string().to_lowercase();
            name_ident = alias;
        }

        let mut assocs_map = BTreeMap::new();

        if let Some(gens) = &mut gens {
            while let Some(GenericArgument::Binding(_)) = gens.last() {
                let v = gens.pop().unwrap().into_value();
                if let GenericArgument::Binding(v) = v {
                    assocs_map.insert(v.ident, v.ty);
                }
            }
        }

        let mut assocs = Punctuated::new();

        for (_, ty) in assocs_map {
            assocs.push_value(GenericArgument::Type(ty.clone()));
            assocs.push_punct(Default::default());
        }

        Self {
            vtbl_name: format_ident!("vtbl_{}", lc_ident),
            lc_name: format_ident!("{}", lc_ident),
            vtbl_typename: format_ident!("{}Vtbl", raw_ident),
            vtbl_get_ident: format_ident!("{}VtblGet", raw_ident),
            assoc_bind_ident: format_ident!("{}AssocBind", raw_ident),
            ret_tmp_typename: format_ident!("{}RetTmp", raw_ident),
            ret_tmp_name: format_ident!("ret_tmp_{}", lc_ident),
            enable_vtbl_name: format_ident!("enable_{}", lc_ident),
            path,
            raw_ident,
            name_ident,
            generics: ParsedGenerics::from(gens.as_ref()),
            assocs: ParsedGenerics::from(&assocs),
        }
    }
}

/// Describes parse trait group, allows to generate code for it.
#[cfg_attr(feature = "unstable", allow(unused))]
pub struct TraitGroup {
    name: Ident,
    cont_name: Ident,
    generics: ParsedGenerics,
    mandatory_vtbl: Vec<TraitInfo>,
    optional_vtbl: Vec<TraitInfo>,
    ext_traits: HashMap<Ident, (Path, ItemTrait)>,
    extra_filler_traits: bool,
}

impl Parse for TraitGroup {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;

        let generics = input.parse()?;

        // TODO: parse associated type defs here
        parse_brace_content(input).ok();

        input.parse::<Token![,]>()?;
        let mandatory_traits = parse_maybe_braced::<AliasPath>(input)?;

        input.parse::<Token![,]>()?;
        let optional_traits = parse_maybe_braced::<AliasPath>(input)?;

        let ext_trait_defs = if input.parse::<Token![,]>().is_ok() {
            parse_maybe_braced::<ItemTrait>(input)?
        } else {
            vec![]
        };

        let mut ext_traits = HashMap::new();

        let mut mandatory_vtbl: Vec<TraitInfo> = mandatory_traits
            .into_iter()
            .map(AliasPath::prelude_remap)
            .map(TraitInfo::from)
            .collect();
        mandatory_vtbl.sort();

        let mut optional_vtbl: Vec<TraitInfo> = optional_traits
            .into_iter()
            .map(AliasPath::prelude_remap)
            .map(TraitInfo::from)
            .collect();
        optional_vtbl.sort();

        let store_exports = get_exports();
        let store_traits = get_store();

        let mut crate_path: Path = parse2(crate_path()).expect("Failed to parse crate path");

        if !crate_path.segments.empty_or_trailing() {
            crate_path.segments.push_punct(Default::default());
        }

        // Go through mand/opt vtbls and pick all external traits used out of there,
        // and then pick add those trait definitions to the ext_traits list from both
        // the input list, and the standard trait collection.
        for vtbl in mandatory_vtbl.iter_mut().chain(optional_vtbl.iter_mut()) {
            let is_ext = match (vtbl.path.leading_colon, vtbl.path.segments.first()) {
                (_, Some(x)) => x.ident == "ext",
                _ => false,
            };

            if !is_ext {
                continue;
            }

            // If the user has supplied a custom implementation.
            if let Some(tr) = ext_trait_defs.iter().find(|tr| tr.ident == vtbl.raw_ident) {
                // Keep the leading colon so as to allow going from the root or relatively
                let leading_colon = std::mem::replace(&mut vtbl.path.leading_colon, None);

                let old_path = std::mem::replace(
                    &mut vtbl.path,
                    Path {
                        leading_colon,
                        segments: Default::default(),
                    },
                );

                for seg in old_path.segments.into_pairs().skip(1) {
                    match seg {
                        punctuated::Pair::Punctuated(p, punc) => {
                            vtbl.path.segments.push_value(p);
                            vtbl.path.segments.push_punct(punc);
                        }
                        punctuated::Pair::End(p) => {
                            vtbl.path.segments.push_value(p);
                        }
                    }
                }

                ext_traits.insert(tr.ident.clone(), (vtbl.path.clone(), tr.clone()));
            } else {
                // Check the store otherwise
                let tr = store_traits
                    .get(&(vtbl.path.clone(), vtbl.raw_ident.clone()))
                    .or_else(|| {
                        store_exports.get(&vtbl.raw_ident).and_then(|p| {
                            vtbl.path = p.clone();
                            store_traits.get(&(p.clone(), vtbl.raw_ident.clone()))
                        })
                    });

                if let Some(tr) = tr {
                    // If we are in the store, we should push crate_path path to the very start
                    let old_path = std::mem::replace(&mut vtbl.path, crate_path.clone());
                    for seg in old_path.segments.into_pairs() {
                        match seg {
                            punctuated::Pair::Punctuated(p, punc) => {
                                vtbl.path.segments.push_value(p);
                                vtbl.path.segments.push_punct(punc);
                            }
                            punctuated::Pair::End(p) => {
                                vtbl.path.segments.push_value(p);
                            }
                        }
                    }
                    ext_traits.insert(tr.ident.clone(), (vtbl.path.clone(), tr.clone()));
                } else {
                    eprintln!(
                        "Could not find external trait {}. Not changing paths.",
                        vtbl.raw_ident
                    );
                }
            }
        }

        let extra_filler_traits = if input.parse::<Token![,]>().is_ok() {
            input.parse::<LitBool>()?.value
        } else {
            true
        };

        let cont_name = format_ident!("{}Container", name);

        Ok(Self {
            name,
            cont_name,
            generics,
            mandatory_vtbl,
            optional_vtbl,
            ext_traits,
            extra_filler_traits,
        })
    }
}

/// Describes trait group to be implemented on a type.
#[cfg(not(feature = "unstable"))]
pub struct TraitGroupImpl {
    ty: Type,
    ty_generics: ParsedGenerics,
    generics: ParsedGenerics,
    group_path: Path,
    group: Ident,
    implemented_vtbl: Vec<TraitInfo>,
    fwd_implemented_vtbl: Option<Vec<TraitInfo>>,
}

#[cfg(not(feature = "unstable"))]
impl Parse for TraitGroupImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut ty: Type = input.parse()?;

        // Parse generic arguments from the type.
        // Here we assume the last instance of AngleBracketed are generic arguments.
        let ty_gens = extract_generics(&mut ty);

        let mut ty_generics = ParsedGenerics::from(ty_gens.as_ref());

        input.parse::<Token![,]>()?;

        let group = input.parse()?;

        let (group_path, group, gens) = split_path_ident(&group)?;

        let generics = ParsedGenerics::from(gens.as_ref());

        let mut generics = match input.parse::<ParsedGenerics>() {
            Ok(ParsedGenerics {
                gen_where_bounds, ..
            }) => {
                parse_brace_content(input).ok();
                ParsedGenerics {
                    gen_where_bounds,
                    ..generics
                }
            }
            _ => generics,
        };

        generics.merge_and_remap(&mut ty_generics);

        let implemented_vtbl = if input.parse::<Token![,]>().is_ok() {
            let implemented_traits = parse_maybe_braced::<AliasPath>(input)?;

            let mut implemented_vtbl: Vec<TraitInfo> = implemented_traits
                .into_iter()
                .map(AliasPath::prelude_remap)
                .map(AliasPath::ext_abs_remap)
                .map(From::from)
                .collect();

            implemented_vtbl.sort();

            implemented_vtbl
        } else {
            vec![]
        };

        let fwd_implemented_vtbl = if input.parse::<Token![,]>().is_ok() {
            let implemented_traits = parse_maybe_braced::<AliasPath>(input)?;

            let mut implemented_vtbl: Vec<TraitInfo> = implemented_traits
                .into_iter()
                .map(AliasPath::prelude_remap)
                .map(AliasPath::ext_abs_remap)
                .map(From::from)
                .collect();

            implemented_vtbl.sort();

            Some(implemented_vtbl)
        } else {
            None
        };

        ty_generics.replace_on_type(&mut ty);
        ty_generics.extract_lifetimes(&ty);

        Ok(Self {
            ty,
            ty_generics,
            generics,
            group_path,
            group,
            implemented_vtbl,
            fwd_implemented_vtbl,
        })
    }
}

#[cfg(not(feature = "unstable"))]
impl TraitGroupImpl {
    /// Generate trait group conversion for a specific type.
    ///
    /// The type will have specified vtables implemented as a conversion function.
    #[cfg(feature = "unstable")]
    pub fn implement_group(&self) -> TokenStream {
        Default::default()
    }

    #[cfg(not(feature = "unstable"))]
    pub fn implement_group(&self) -> TokenStream {
        let crate_path = crate_path();

        let ctx_bound = super::traits::ctx_bound();

        let ty = &self.ty;

        let group = &self.group;
        let group_path = &self.group_path;
        let ParsedGenerics { gen_use, .. } = &self.generics;

        let ParsedGenerics {
            gen_declare,
            gen_where_bounds,
            mut life_declare,
            mut life_use,
            ..
        } = [&self.ty_generics, &self.generics]
            .iter()
            .copied()
            .collect();

        // If no lifetimes are used, default to 'cglue_a
        if life_use.is_empty() {
            assert!(life_declare.is_empty());
            let lifetime = Lifetime {
                apostrophe: proc_macro2::Span::call_site(),
                ident: format_ident!("cglue_a"),
            };
            life_use.push_value(lifetime.clone());
            life_declare.push_value(LifetimeDef {
                lifetime,
                attrs: Default::default(),
                bounds: Default::default(),
                colon_token: Default::default(),
            });
        }

        if !life_declare.trailing_punct() {
            life_declare.push_punct(Default::default());
        }

        if !life_use.trailing_punct() {
            life_use.push_punct(Default::default());
        }

        // Lifetime should always exist based on previous code
        let first_life = life_use.first().unwrap();

        let gen_lt_bounds = self.generics.declare_lt_for_all(&quote!(#first_life));
        let gen_sabi_bounds = self.generics.declare_sabi_for_all(&crate_path);

        let gen_where_bounds = quote! {
            #gen_where_bounds
            #gen_sabi_bounds
            #gen_lt_bounds
        };

        let filler_trait = format_ident!("{}VtableFiller", group);
        let vtable_type = format_ident!("{}Vtables", group);
        let cont_name = format_ident!("{}Container", group);

        let implemented_tables = TraitGroup::enable_opt_vtbls(self.implemented_vtbl.iter());
        let vtbl_where_bounds = TraitGroup::vtbl_where_bounds(
            self.implemented_vtbl.iter(),
            &cont_name,
            quote!(CGlueInst),
            quote!(CGlueCtx),
            &self.generics,
            Some(quote!(Self)).as_ref(),
            first_life,
        );

        let gen = quote! {
            impl<#life_declare CGlueInst: ::core::ops::Deref<Target = #ty>, CGlueCtx: #ctx_bound, #gen_declare>
                #group_path #filler_trait<#life_use CGlueInst, CGlueCtx, #gen_use> for #ty
            where #gen_where_bounds #vtbl_where_bounds {
                fn fill_table(table: #group_path #vtable_type<#life_use CGlueInst, CGlueCtx, #gen_use>) -> #group_path #vtable_type<#life_use CGlueInst, CGlueCtx, #gen_use> {
                    table #implemented_tables
                }
            }
        };

        if let Some(fwd_vtbl) = &self.fwd_implemented_vtbl {
            let fwd_filler_trait = format_ident!("{}FwdVtableFiller", group);

            let fwd_ty = quote!(#crate_path::forward::Fwd<&#first_life mut #ty>);

            let implemented_tables = TraitGroup::enable_opt_vtbls(fwd_vtbl.iter());
            let vtbl_where_bounds = TraitGroup::vtbl_where_bounds(
                fwd_vtbl.iter(),
                &cont_name,
                quote!(CGlueInst),
                quote!(CGlueCtx),
                &self.generics,
                Some(quote!(Self)).as_ref(),
                first_life,
            );

            quote! {
                #gen

                impl<#life_declare CGlueInst: ::core::ops::Deref<Target = #fwd_ty>, CGlueCtx: #ctx_bound, #gen_declare>
                    #group_path #fwd_filler_trait<#life_use CGlueInst, CGlueCtx, #gen_use> for #ty
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #crate_path::trait_group::CGlueObjBase,
                    #gen_where_bounds #vtbl_where_bounds
                {
                    fn fill_fwd_table(table: #group_path #vtable_type<#life_use CGlueInst, CGlueCtx, #gen_use>) -> #group_path #vtable_type<#life_use CGlueInst, CGlueCtx, #gen_use> {
                        table #implemented_tables
                    }
                }
            }
        } else {
            gen
        }
    }
}

pub struct TraitCastGroup {
    name: TokenStream,
    needed_vtbls: Vec<TraitInfo>,
}

pub enum CastType {
    Cast,
    AsRef,
    AsMut,
    Into,
    OnlyCheck,
}

impl Parse for TraitCastGroup {
    fn parse(input: ParseStream) -> Result<Self> {
        let name;

        if let Ok(expr) = input.parse::<Expr>() {
            name = quote!(#expr);
        } else {
            name = input.parse::<Ident>()?.into_token_stream();
        }

        let implemented_traits = input.parse::<TypeImplTrait>()?;

        let mut needed_vtbls: Vec<TraitInfo> = implemented_traits
            .bounds
            .into_iter()
            .filter_map(|b| match b {
                TypeParamBound::Trait(tr) => Some(AliasPath {
                    path: tr.path,
                    alias: None,
                }),
                _ => None,
            })
            .map(From::from)
            .collect();

        needed_vtbls.sort();

        Ok(Self { name, needed_vtbls })
    }
}

impl TraitCastGroup {
    /// Generate a cast to a specific type.
    ///
    /// The type will have specified vtables implemented as a conversion function.
    pub fn cast_group(&self, cast: CastType) -> TokenStream {
        let prefix = match cast {
            CastType::Cast => "cast",
            CastType::AsRef => "as_ref",
            CastType::AsMut => "as_mut",
            CastType::Into => "into",
            CastType::OnlyCheck => "check",
        };

        let name = &self.name;
        let func_name = TraitGroup::optional_func_name(prefix, self.needed_vtbls.iter());

        quote! {
            (#name).#func_name()
        }
    }
}

impl TraitGroup {
    /// Identifier for optional group struct.
    ///
    /// # Arguments
    ///
    /// * `name` - base name of the trait group.
    /// * `postfix` - postfix to add after the naem, and before `With`.
    /// * `traits` - traits that are to be implemented.
    pub fn optional_group_ident<'a>(
        name: &Ident,
        postfix: &str,
        traits: impl Iterator<Item = &'a TraitInfo>,
    ) -> Ident {
        let mut all_traits = String::new();

        for TraitInfo { name_ident, .. } in traits {
            all_traits.push_str(&name_ident.to_string());
        }

        format_ident!("{}{}With{}", name, postfix, all_traits)
    }

    /// Get the name of the function for trait conversion.
    ///
    /// # Arguments
    ///
    /// * `prefix` - function name prefix.
    /// * `lc_names` - lowercase identifiers of the traits the function implements.
    pub fn optional_func_name<'a>(
        prefix: &str,
        lc_names: impl Iterator<Item = &'a TraitInfo>,
    ) -> Ident {
        let mut ident = format_ident!("{}_impl", prefix);

        for TraitInfo { lc_name, .. } in lc_names {
            ident = format_ident!("{}_{}", ident, lc_name);
        }

        ident
    }

    /// Generate function calls that enable individual functional vtables.
    ///
    /// # Arguments
    ///
    /// * `iter` - iterator of optional traits to enable
    pub fn enable_opt_vtbls<'a>(iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            enable_vtbl_name, ..
        } in iter
        {
            ret.extend(quote!(.#enable_vtbl_name()));
        }

        ret
    }

    /// Generate full code for the trait group.
    ///
    /// This trait group will have all variants generated for converting, building, and
    /// converting it.
    pub fn create_group(&self) -> TokenStream {
        // Path to trait group import.
        let crate_path = crate::util::crate_path();

        let ctx_bound = super::traits::ctx_bound();

        let trg_path: TokenStream = quote!(#crate_path::trait_group);

        let c_void = crate::util::void_type();

        let name = &self.name;
        let cont_name = &self.cont_name;

        let ParsedGenerics {
            gen_declare,
            gen_use,
            gen_where_bounds,
            ..
        } = &self.generics;

        let gen_lt_bounds = self.generics.declare_lt_for_all(&quote!('cglue_a));
        let gen_sabi_bounds = self.generics.declare_sabi_for_all(&crate_path);

        // Structures themselves do not need StableAbi bounds, if layout_checks is on
        let gen_where_bounds_base = quote! {
            #gen_where_bounds
            #gen_lt_bounds
        };

        // If layout_checks is enabled, this will include StableAbi bounds
        let gen_where_bounds = quote! {
            #gen_where_bounds_base
            #gen_sabi_bounds
        };

        let cglue_a_lifetime = Lifetime {
            apostrophe: proc_macro2::Span::call_site(),
            ident: format_ident!("cglue_a"),
        };

        let mandatory_vtbl_defs = self.mandatory_vtbl_defs(self.mandatory_vtbl.iter());
        let optional_vtbl_defs = self.optional_vtbl_defs(quote!(CGlueInst), quote!(CGlueCtx));
        let optional_vtbl_defs_boxed = self.optional_vtbl_defs(
            quote!(#crate_path::boxed::CBox<'cglue_a, CGlueT>),
            quote!(#crate_path::trait_group::NoContext),
        );

        let mand_vtbl_default = self.mandatory_vtbl_defaults();
        let mand_ret_tmp_default = self.mandatory_ret_tmp_defaults();
        let full_opt_ret_tmp_default = Self::ret_tmp_defaults(self.optional_vtbl.iter());
        let default_opt_vtbl_list = self.default_opt_vtbl_list();
        let mand_vtbl_list = self.vtbl_list(self.mandatory_vtbl.iter());
        let full_opt_vtbl_list = self.vtbl_list(self.optional_vtbl.iter());
        let mandatory_as_ref_impls = self.mandatory_as_ref_impls(&trg_path);

        let get_container_impl = self.get_container_impl(name, &trg_path, &self.generics);

        let mandatory_internal_trait_impls = self.internal_trait_impls(
            name,
            self.mandatory_vtbl.iter(),
            &self.generics,
            &crate_path,
        );
        let vtbl_where_bounds = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            cont_name,
            quote!(CGlueInst),
            quote!(CGlueCtx),
            &self.generics,
            None,
            &cglue_a_lifetime,
        );
        let vtbl_where_bounds_noctx = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            cont_name,
            quote!(CGlueInst),
            quote!(#trg_path::NoContext),
            &self.generics,
            None,
            &cglue_a_lifetime,
        );
        let vtbl_where_bounds_boxed = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            cont_name,
            quote!(#crate_path::boxed::CBox<'cglue_a, CGlueT>),
            quote!(#crate_path::trait_group::NoContext),
            &self.generics,
            None,
            &cglue_a_lifetime,
        );
        let vtbl_where_bounds_ctxboxed = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            cont_name,
            quote!(#crate_path::boxed::CBox<'cglue_a, CGlueT>),
            quote!(CGlueCtx),
            &self.generics,
            None,
            &cglue_a_lifetime,
        );
        let ret_tmp_defs = self.ret_tmp_defs(self.optional_vtbl.iter());

        let mut enable_funcs = TokenStream::new();
        let mut enable_funcs_vtbl = TokenStream::new();

        #[cfg(feature = "layout_checks")]
        let derive_layouts = quote!(#[derive(::abi_stable::StableAbi)]);
        #[cfg(not(feature = "layout_checks"))]
        let derive_layouts = quote!();

        let all_gen_use = &gen_use;

        // Work around needless_update lint
        let fill_rest = if self.optional_vtbl.len() + self.mandatory_vtbl.len() > 1 {
            quote!(..self)
        } else {
            quote!()
        };

        for TraitInfo {
            enable_vtbl_name,
            vtbl_typename,
            vtbl_name,
            path,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in &self.optional_vtbl
        {
            for (funcs, fill_rest) in &mut [
                (&mut enable_funcs, &quote!(..self)),
                (&mut enable_funcs_vtbl, &fill_rest),
            ] {
                funcs.extend(quote! {
                    pub fn #enable_vtbl_name (self) -> Self
                        where &'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use>: Default {
                            Self {
                                #vtbl_name: Some(Default::default()),#fill_rest
                            }
                    }
                });
            }
        }

        let mut trait_funcs = TokenStream::new();

        let mut opt_structs = TokenStream::new();
        let mut opt_struct_imports = TokenStream::new();

        let impl_traits =
            self.impl_traits(self.mandatory_vtbl.iter().chain(self.optional_vtbl.iter()));

        let base_doc = format!(
            " Trait group potentially implementing `{}` traits.",
            impl_traits
        );
        let trback_doc = format!("be transformed back into `{}` without losing data.", name);
        let new_doc = format!(" Create new instance of {}.", name);

        let base_name = format_ident!("{}Base", name);
        let base_name_ref = format_ident!("{}BaseRef", name);
        let base_name_ctx_ref = format_ident!("{}BaseCtxRef", name);
        let base_name_arc_ref = format_ident!("{}BaseArcRef", name);
        let base_name_mut = format_ident!("{}BaseMut", name);
        let base_name_ctx_mut = format_ident!("{}BaseCtxMut", name);
        let base_name_arc_mut = format_ident!("{}BaseArcMut", name);
        let base_name_boxed = format_ident!("{}BaseBox", name);
        let base_name_arc_box = format_ident!("{}BaseArcBox", name);
        let base_name_ctx_box = format_ident!("{}BaseCtxBox", name);
        let opaque_name_ref = format_ident!("{}Ref", name);
        let opaque_name_ctx_ref = format_ident!("{}CtxRef", name);
        let opaque_name_arc_ref = format_ident!("{}ArcRef", name);
        let opaque_name_mut = format_ident!("{}Mut", name);
        let opaque_name_ctx_mut = format_ident!("{}CtxMut", name);
        let opaque_name_arc_mut = format_ident!("{}ArcMut", name);
        let opaque_name_boxed = format_ident!("{}Box", name);
        let opaque_name_arc_box = format_ident!("{}ArcBox", name);
        let opaque_name_ctx_box = format_ident!("{}CtxBox", name);

        #[cfg(not(feature = "unstable"))]
        let filler_trait = format_ident!("{}VtableFiller", name);
        #[cfg(not(feature = "unstable"))]
        let fwd_filler_trait = format_ident!("{}FwdVtableFiller", name);
        let vtable_type = format_ident!("{}Vtables", name);

        for traits in self
            .optional_vtbl
            .iter()
            .powerset()
            .filter(|v| !v.is_empty())
        {
            let func_name = Self::optional_func_name("cast", traits.iter().copied());
            let func_name_final = Self::optional_func_name("into", traits.iter().copied());
            let func_name_check = Self::optional_func_name("check", traits.iter().copied());
            let func_name_mut = Self::optional_func_name("as_mut", traits.iter().copied());
            let func_name_ref = Self::optional_func_name("as_ref", traits.iter().copied());
            let opt_final_name = Self::optional_group_ident(name, "Final", traits.iter().copied());
            let opt_name = Self::optional_group_ident(name, "", traits.iter().copied());
            let opt_vtbl_defs = self.mandatory_vtbl_defs(traits.iter().copied());
            let opt_mixed_vtbl_defs = self.mixed_opt_vtbl_defs(traits.iter().copied());

            let opt_vtbl_list = self.vtbl_list(traits.iter().copied());
            let opt_vtbl_unwrap = self.vtbl_unwrap_list(traits.iter().copied());
            let opt_vtbl_unwrap_validate = self.vtbl_unwrap_validate(traits.iter().copied());

            let mixed_opt_vtbl_unwrap = self.mixed_opt_vtbl_unwrap_list(traits.iter().copied());

            let get_container_impl = self.get_container_impl(&opt_name, &trg_path, &self.generics);

            let opt_as_ref_impls = self.as_ref_impls(
                &opt_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &trg_path,
            );

            let opt_internal_trait_impls = self.internal_trait_impls(
                &opt_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &crate_path,
            );

            let get_container_impl_final =
                self.get_container_impl(&opt_final_name, &trg_path, &self.generics);

            let opt_final_as_ref_impls = self.as_ref_impls(
                &opt_final_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &trg_path,
            );

            let opt_final_internal_trait_impls = self.internal_trait_impls(
                &opt_final_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &crate_path,
            );

            let impl_traits =
                self.impl_traits(self.mandatory_vtbl.iter().chain(traits.iter().copied()));

            let opt_final_doc = format!(
                " Final {} variant with `{}` implemented.",
                name, &impl_traits
            );
            let opt_final_doc2 = format!(
                " Retrieve this type using [`{}`]({}::{}) function.",
                func_name_final, name, func_name_final
            );

            let opt_doc = format!(
                " Concrete {} variant with `{}` implemented.",
                name, &impl_traits
            );
            let opt_doc2 = format!(" Retrieve this type using one of [`{}`]({}::{}), [`{}`]({}::{}), or [`{}`]({}::{}) functions.", func_name, name, func_name, func_name_mut, name, func_name_mut, func_name_ref, name, func_name_ref);

            // TODO: remove unused generics to remove need for phantom data

            opt_struct_imports.extend(quote! {
                #opt_final_name,
                #opt_name,
            });

            opt_structs.extend(quote! {

                // Final implementation - more compact layout.

                #[doc = #opt_final_doc]
                ///
                #[doc = #opt_final_doc2]
                #[repr(C)]
                #derive_layouts
                pub struct #opt_final_name<'cglue_a, CGlueInst: 'cglue_a, CGlueCtx: #ctx_bound, #gen_declare>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds_base
                {
                    #mandatory_vtbl_defs
                    #opt_vtbl_defs
                    container: #cont_name<CGlueInst, CGlueCtx, #gen_use>,
                }

                #get_container_impl_final

                #opt_final_as_ref_impls

                #opt_final_internal_trait_impls

                // Non-final implementation. Has the same layout as the base struct.

                #[doc = #opt_doc]
                ///
                #[doc = #opt_doc2]
                #[repr(C)]
                #derive_layouts
                pub struct #opt_name<'cglue_a, CGlueInst: 'cglue_a, CGlueCtx: #ctx_bound, #gen_declare>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds_base
                {
                    #mandatory_vtbl_defs
                    #opt_mixed_vtbl_defs
                    container: #cont_name<CGlueInst, CGlueCtx, #gen_use>,
                }

                unsafe impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare>
                    #trg_path::Opaquable for #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    type OpaqueTarget = #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>;
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare>
                    From<#opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>> for #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    fn from(input: #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>) -> Self {
                        #trg_path::Opaquable::into_opaque(input)
                    }
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare> #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                    where Self: #trg_path::Opaquable,
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    /// Cast back into the original group
                    pub fn upcast(self) -> <Self as #trg_path::Opaquable>::OpaqueTarget {
                        #trg_path::Opaquable::into_opaque(self)
                    }
                }

                #get_container_impl

                #opt_as_ref_impls

                #opt_internal_trait_impls
            });

            let func_final_doc1 = format!(
                " Retrieve a final {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_final_doc2 = format!(
                " This consumes the `{}`, and outputs `Some(impl {})`, if all types are present.",
                name, impl_traits
            );

            let func_doc1 = format!(
                " Retrieve a concrete {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_doc2 = format!(" This consumes the `{}`, and outputs `Some(impl {})`, if all types are present. It is possible to cast this type back with the `From` implementation.", name, impl_traits);

            let func_check_doc1 = format!(" Check whether {} implements `{}`.", name, impl_traits);
            let func_check_doc2 =
                " If this check returns true, it is safe to run consuming conversion operations."
                    .to_string();

            let func_mut_doc1 = format!(
                " Retrieve mutable reference to a concrete {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_ref_doc1 = format!(
                " Retrieve immutable reference to a concrete {} variant that implements `{}`.",
                name, impl_traits
            );

            trait_funcs.extend(quote! {
                #[doc = #func_check_doc1]
                ///
                #[doc = #func_check_doc2]
                pub fn #func_name_check(&self) -> bool
                    where #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>: 'cglue_a + #impl_traits
                {
                    self.#func_name_ref().is_some()
                }

                #[doc = #func_final_doc1]
                ///
                #[doc = #func_final_doc2]
                pub fn #func_name_final(self) -> ::core::option::Option<impl 'cglue_a + #impl_traits>
                    where #opt_final_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        container,
                        #mand_vtbl_list
                        #opt_vtbl_list
                        ..
                    } = self;

                    Some(#opt_final_name {
                        container,
                        #mand_vtbl_list
                        #opt_vtbl_unwrap
                    })
                }

                #[doc = #func_doc1]
                ///
                #[doc = #func_doc2]
                pub fn #func_name(self) -> ::core::option::Option<#opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>>
                    where #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        container,
                        #mand_vtbl_list
                        #full_opt_vtbl_list
                    } = self;

                    Some(#opt_name {
                        container,
                        #mand_vtbl_list
                        #mixed_opt_vtbl_unwrap
                    })
                }

                #[doc = #func_mut_doc1]
                pub fn #func_name_mut<'b>(&'b mut self) -> ::core::option::Option<&'b mut (impl 'cglue_a + #impl_traits)>
                    where #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        container,
                        #mand_vtbl_list
                        #opt_vtbl_list
                        ..
                    } = self;

                    let _ = (#opt_vtbl_unwrap_validate);

                    // Safety:
                    //
                    // Structure layouts are fully compatible,
                    // optional reference validity was checked beforehand

                    unsafe {
                        (self as *mut Self as *mut #opt_name<CGlueInst, CGlueCtx, #gen_use>).as_mut()
                    }
                }

                #[doc = #func_ref_doc1]
                pub fn #func_name_ref<'b>(&'b self) -> ::core::option::Option<&'b (impl 'cglue_a + #impl_traits)>
                    where #opt_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        #mand_vtbl_list
                        #opt_vtbl_list
                        ..
                    } = self;

                    let _ = (#opt_vtbl_unwrap_validate);

                    // Safety:
                    //
                    // Structure layouts are fully compatible,
                    // optional reference validity was checked beforehand

                    unsafe {
                        (self as *const Self as *const #opt_name<CGlueInst, CGlueCtx, #gen_use>).as_ref()
                    }
                }
            });
        }

        #[cfg(not(feature = "unstable"))]
        let (extra_filler_traits, filler_trait_imports) = if self.extra_filler_traits {
            let traits = quote! {
                pub trait #fwd_filler_trait<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #gen_declare>: 'cglue_a + Sized
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    fn fill_fwd_table(table: #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>) -> #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>;
                }

                impl<'cglue_a, CGlueInst: ::core::ops::Deref<Target = #crate_path::forward::Fwd<&'cglue_a mut CGlueT>>, CGlueT, CGlueCtx: #ctx_bound, #gen_declare>
                    #filler_trait<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                    for #crate_path::forward::Fwd<&'cglue_a mut CGlueT>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    CGlueT: #fwd_filler_trait<'cglue_a, CGlueInst, CGlueCtx, #gen_use>,
                    #gen_where_bounds
                {
                    fn fill_table(table: #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>) -> #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use> {
                        CGlueT::fill_fwd_table(table)
                    }
                }
            };

            let imports = quote! {
                #filler_trait,
                #fwd_filler_trait,
            };

            (traits, imports)
        } else {
            (quote!(), quote!(#filler_trait,))
        };

        #[cfg(feature = "unstable")]
        let filler_trait_imports = quote!();

        let submod_name = format_ident!("cglue_{}", name.to_string().to_lowercase());

        let cglue_obj_impl = self.cglue_obj_impl(&trg_path, &self.generics);

        #[cfg(feature = "unstable")]
        let cglue_inst_filler_trait_bound = quote!();
        #[cfg(not(feature = "unstable"))]
        let cglue_inst_filler_trait_bound =
            quote!(CGlueInst::Target: #filler_trait<'cglue_a, CGlueInst, CGlueCtx, #gen_use>,);
        #[cfg(feature = "unstable")]
        let create_vtbl = quote!(Default::default());
        #[cfg(not(feature = "unstable"))]
        let create_vtbl = quote!(CGlueInst::Target::fill_table(Default::default()));

        #[cfg(feature = "unstable")]
        let filler_trait_impl = quote!();
        #[cfg(not(feature = "unstable"))]
        let filler_trait_impl = quote! {
            pub trait #filler_trait<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare>: Sized
            where
                #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                #gen_where_bounds
            {
                fn fill_table(table: #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>) -> #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>;
            }

            #extra_filler_traits
        };

        quote! {

            #[doc(hidden)]
            pub use #submod_name::*;

            pub mod #submod_name {
                use super::*;

                pub use cglue_internal::{
                    #name,
                    #vtable_type,
                    #filler_trait_imports
                    #base_name,
                    #base_name_ref,
                    #base_name_ctx_ref,
                    #base_name_arc_ref,
                    #base_name_mut,
                    #base_name_ctx_mut,
                    #base_name_arc_mut,
                    #base_name_boxed,
                    #base_name_arc_box,
                    #base_name_ctx_box,
                    #opaque_name_ref,
                    #opaque_name_ctx_ref,
                    #opaque_name_arc_ref,
                    #opaque_name_mut,
                    #opaque_name_ctx_mut,
                    #opaque_name_arc_mut,
                    #opaque_name_boxed,
                    #opaque_name_arc_box,
                    #opaque_name_ctx_box,
                    #cont_name,
                    #opt_struct_imports
                };

                mod cglue_internal {
                use super::*;

                #[repr(C)]
                #[doc = #base_doc]
                ///
                /// Optional traits are not implemented here, however. There are numerous conversion
                /// functions available for safely retrieving a concrete collection of traits.
                ///
                /// `check_impl_` functions allow to check if the object implements the wanted traits.
                ///
                /// `into_impl_` functions consume the object and produce a new final structure that
                /// keeps only the required information.
                ///
                /// `cast_impl_` functions merely check and transform the object into a type that can
                #[doc = #trback_doc]
                ///
                /// `as_ref_`, and `as_mut_` functions obtain references to safe objects, but do not
                /// perform any memory transformations either. They are the safest to use, because
                /// there is no risk of accidentally consuming the whole object.
                #derive_layouts
                pub struct #name<'cglue_a, CGlueInst: 'cglue_a, CGlueCtx: #ctx_bound, #gen_declare>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds_base
                {
                    #mandatory_vtbl_defs
                    #optional_vtbl_defs
                    container: #cont_name<CGlueInst, CGlueCtx, #gen_use>,
                }

                #get_container_impl

                #[repr(C)]
                #derive_layouts
                pub struct #cont_name<CGlueInst, CGlueCtx: #ctx_bound, #gen_declare>
                {
                    instance: CGlueInst,
                    context: CGlueCtx,
                    #ret_tmp_defs
                }

                #cglue_obj_impl

                unsafe impl<CGlueInst: #trg_path::Opaquable, CGlueCtx: #ctx_bound, #gen_declare>
                    #trg_path::Opaquable for #cont_name<CGlueInst, CGlueCtx, #gen_use>
                {
                    type OpaqueTarget = #cont_name<CGlueInst::OpaqueTarget, CGlueCtx, #gen_use>;
                }

                #[repr(C)]
                #derive_layouts
                pub struct #vtable_type<'cglue_a, CGlueInst: 'cglue_a, CGlueCtx: #ctx_bound, #gen_declare>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds_base
                {
                    #mandatory_vtbl_defs
                    #optional_vtbl_defs
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare> Default
                    for #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #vtbl_where_bounds #gen_where_bounds
                {
                    fn default() -> Self {
                        Self {
                            #mand_vtbl_default
                            #default_opt_vtbl_list
                        }
                    }
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare> #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    #enable_funcs
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare> #vtable_type<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    #enable_funcs_vtbl
                }

                #filler_trait_impl

                pub type #base_name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                    = #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>;

                pub type #base_name_boxed<'cglue_a, CGlueT, #gen_use>
                    = #base_name_ctx_box<'cglue_a, CGlueT, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_ctx_box<'cglue_a, CGlueT, CGlueCtx, #gen_use>
                    = #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx, #gen_use>;

                pub type #base_name_arc_box<'cglue_a, CGlueT, CGlueArcTy, #gen_use>
                    = #base_name_ctx_box<'cglue_a, CGlueT, #crate_path::arc::CArc<CGlueArcTy>, #gen_use>;

                pub type #base_name_ref<'cglue_a, CGlueT, #gen_use>
                    = #name<'cglue_a, &'cglue_a CGlueT, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_ctx_ref<'cglue_a, CGlueT, CGlueCtx, #gen_use>
                    = #name<'cglue_a, &'cglue_a CGlueT, CGlueCtx, #gen_use>;

                pub type #base_name_arc_ref<'cglue_a, CGlueT, CGlueArcTy, #gen_use>
                    = #name<'cglue_a, &'cglue_a CGlueT, #crate_path::arc::CArc<CGlueArcTy>, #gen_use>;

                pub type #base_name_mut<'cglue_a, CGlueT, #gen_use>
                    = #name<'cglue_a, &'cglue_a mut CGlueT, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_ctx_mut<'cglue_a, CGlueT, CGlueCtx, #gen_use>
                    = #name<'cglue_a, &'cglue_a mut CGlueT, CGlueCtx, #gen_use>;

                pub type #base_name_arc_mut<'cglue_a, CGlueT, CGlueArcTy, #gen_use>
                    = #name<'cglue_a, &'cglue_a mut CGlueT, #crate_path::arc::CArc<CGlueArcTy>, #gen_use>;

                pub type #opaque_name_boxed<'cglue_a, #gen_use>
                    = #base_name_boxed<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_ref<'cglue_a, #gen_use>
                    = #base_name_ref<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_ctx_ref<'cglue_a, CGlueCtx, #gen_use>
                    = #base_name_ctx_ref<'cglue_a, #c_void, CGlueCtx, #gen_use>;

                pub type #opaque_name_arc_ref<'cglue_a, #gen_use>
                    = #base_name_arc_ref<'cglue_a, #c_void, #c_void, #gen_use>;

                pub type #opaque_name_mut<'cglue_a, #gen_use>
                    = #base_name_mut<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_ctx_mut<'cglue_a, CGlueCtx, #gen_use>
                    = #base_name_ctx_mut<'cglue_a, #c_void, CGlueCtx, #gen_use>;

                pub type #opaque_name_arc_mut<'cglue_a, #gen_use>
                    = #base_name_arc_mut<'cglue_a, #c_void, #c_void, #gen_use>;

                pub type #opaque_name_ctx_box<'cglue_a, CGlueCtx, #gen_use>
                    = #base_name_ctx_box<'cglue_a, #c_void, CGlueCtx, #gen_use>;

                pub type #opaque_name_arc_box<'cglue_a, #gen_use>
                    = #base_name_arc_box<'cglue_a, #c_void, #c_void, #gen_use>;


                impl<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #gen_declare>
                    From<(CGlueInst, CGlueCtx)> for #cont_name<CGlueInst, CGlueCtx, #gen_use>
                where
                    Self: #trg_path::CGlueObjBase
                {
                    fn from((instance, context): (CGlueInst, CGlueCtx)) -> Self {
                        Self {
                            instance,
                            context,
                            #mand_ret_tmp_default
                            #full_opt_ret_tmp_default
                        }
                    }
                }

                impl<'cglue_a, CGlueT, CGlueCtx: #ctx_bound, #gen_declare>
                    From<(CGlueT, CGlueCtx)> for #cont_name<#crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx, #gen_use>
                where
                    Self: #trg_path::CGlueObjBase
                {
                    fn from((this, context): (CGlueT, CGlueCtx)) -> Self {
                        Self::from((#crate_path::boxed::CBox::from(this), context))
                    }
                }

                impl<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #gen_declare>
                    From<#cont_name<CGlueInst, CGlueCtx, #gen_use>> for #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #cglue_inst_filler_trait_bound
                    #vtbl_where_bounds #gen_where_bounds
                {
                    fn from(container: #cont_name<CGlueInst, CGlueCtx, #gen_use>) -> Self {
                        let vtbl = #create_vtbl;

                        let #vtable_type {
                            #mand_vtbl_list
                            #full_opt_vtbl_list
                        } = vtbl;

                        Self {
                            container,
                            #mand_vtbl_list
                            #full_opt_vtbl_list
                        }
                    }
                }

                impl<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #gen_declare>
                    From<(CGlueInst, CGlueCtx)> for #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    Self: From<#cont_name<CGlueInst, CGlueCtx, #gen_use>>,
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #vtbl_where_bounds #gen_where_bounds
                {
                    fn from((instance, context): (CGlueInst, CGlueCtx)) -> Self {
                        Self::from(#cont_name::from((instance, context)))
                    }
                }

                impl<'cglue_a, CGlueT, #gen_declare>
                    From<CGlueT> for #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, #crate_path::trait_group::NoContext, #gen_use>
                where
                    Self: From<(#crate_path::boxed::CBox<'cglue_a, CGlueT>, #crate_path::trait_group::NoContext)>,
                    #vtbl_where_bounds_boxed #gen_where_bounds
                {
                    fn from(instance: CGlueT) -> Self {
                        Self::from((#crate_path::boxed::CBox::from(instance), Default::default()))
                    }
                }

                impl<'cglue_a, CGlueInst: core::ops::Deref, #gen_declare> From<CGlueInst>
                    for #name<'cglue_a, CGlueInst, #trg_path::NoContext, #gen_use>
                where
                    Self: From<(CGlueInst, #crate_path::trait_group::NoContext)>,
                    #cont_name<CGlueInst, #trg_path::NoContext, #gen_use>: #trg_path::CGlueObjBase,
                    #vtbl_where_bounds_noctx #gen_where_bounds
                {
                    fn from(instance: CGlueInst) -> Self {
                        Self::from((instance, Default::default()))
                    }
                }

                impl<'cglue_a, CGlueT, CGlueCtx: #ctx_bound, #gen_declare> From<(CGlueT, CGlueCtx)>
                    for #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx, #gen_use>
                where
                    Self: From<(#crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx)>,
                    #cont_name<#crate_path::boxed::CBox<'cglue_a, CGlueT>, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #vtbl_where_bounds_ctxboxed #gen_where_bounds
                {
                    fn from((this, context): (CGlueT, CGlueCtx)) -> Self {
                        Self::from((#crate_path::boxed::CBox::from(this), context))
                    }
                }

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_declare>
                    #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #vtbl_where_bounds #gen_where_bounds
                {
                    #[doc = #new_doc]
                    pub fn new(instance: CGlueInst, context: CGlueCtx, #optional_vtbl_defs) -> Self
                        where #vtbl_where_bounds
                    {
                        Self {
                            container: #cont_name {
                                instance,
                                context,
                                #mand_ret_tmp_default
                                #full_opt_ret_tmp_default
                            },
                            #mand_vtbl_default
                            #full_opt_vtbl_list
                        }
                    }
                }

                impl<'cglue_a, CGlueT, #gen_declare> #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueT>, #crate_path::trait_group::NoContext, #gen_use>
                    where #gen_where_bounds
                {
                    #[doc = #new_doc]
                    ///
                    /// `instance` will be moved onto heap.
                    pub fn new_boxed(this: CGlueT, #optional_vtbl_defs_boxed) -> Self
                        where #vtbl_where_bounds_boxed
                    {
                        Self::new(From::from(this), Default::default(), #full_opt_vtbl_list)
                    }
                }

                /// Convert into opaque object.
                ///
                /// This is the prerequisite for using underlying trait implementations.
                unsafe impl<'cglue_a, CGlueInst: #trg_path::Opaquable, CGlueCtx: #ctx_bound, #gen_declare>
                    #trg_path::Opaquable for #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #cont_name<CGlueInst::OpaqueTarget, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    type OpaqueTarget = #name<'cglue_a, CGlueInst::OpaqueTarget, CGlueCtx, #gen_use>;
                }

                impl<
                    'cglue_a,
                    CGlueInst, //: ::core::ops::Deref
                    CGlueCtx: #ctx_bound,
                    #gen_declare
                >
                    #name<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                    #gen_where_bounds
                {
                    #trait_funcs
                }

                #mandatory_as_ref_impls

                #mandatory_internal_trait_impls

                #opt_structs
            }
            }
        }
    }

    fn internal_trait_impls<'a>(
        &'a self,
        self_ident: &Ident,
        iter: impl Iterator<Item = &'a TraitInfo>,
        all_generics: &ParsedGenerics,
        crate_path: &TokenStream,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        let cont_name = &self.cont_name;

        let ctx_bound = super::traits::ctx_bound();

        let ParsedGenerics { gen_use, .. } = all_generics;

        for TraitInfo {
            path,
            raw_ident,
            generics:
                ParsedGenerics {
                    life_use: tr_life_use,
                    gen_use: tr_gen_use,
                    ..
                },
            ..
        } in iter
        {
            if let Some((ext_path, tr_info)) = self.ext_traits.get(raw_ident) {
                let mut impls = TokenStream::new();

                let ext_name = format_ident!("{}Ext", raw_ident);

                let (funcs, _, (_, assoc_idents, _), _) = super::traits::parse_trait(
                    tr_info,
                    crate_path,
                    false,
                    super::traits::process_item,
                );

                for a in &assoc_idents {
                    impls.extend(
                        quote!(type #a = <Self as #ext_name<#tr_life_use #tr_gen_use>>::#a;),
                    );
                }

                for func in &funcs {
                    func.int_trait_impl(Some(ext_path), &ext_name, &mut impls);
                }

                let gen = quote! {
                    impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #gen_use>
                        #path #raw_ident <#tr_life_use #tr_gen_use> for #self_ident<'cglue_a, CGlueInst, CGlueCtx, #gen_use>
                    where
                        #cont_name<CGlueInst, CGlueCtx, #gen_use>: #crate_path::trait_group::CGlueObjBase,
                        Self: #ext_path #ext_name<#tr_life_use #tr_gen_use>
                    {
                        #impls
                    }
                };

                ret.extend(gen);
            }
        }

        ret
    }

    /// Required vtable definitions.
    ///
    /// Required means they must be valid - non-Option.
    ///
    /// # Arguments
    ///
    /// * `iter` - can be any list of traits.
    ///
    fn mandatory_vtbl_defs<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        let cont_name = &self.cont_name;

        let all_gen_use = &self.generics.gen_use;

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in iter
        {
            ret.extend(
                quote!(#vtbl_name: &'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use>, ),
            );
        }

        ret
    }

    /// Get a sequence of `Trait1 + Trait2 + Trait3 ...`
    ///
    /// # Arguments
    ///
    /// * `traits` - traits to combine.
    fn impl_traits<'a>(&'a self, traits: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for (
            i,
            TraitInfo {
                path,
                raw_ident,
                generics:
                    ParsedGenerics {
                        life_use, gen_use, ..
                    },
                ..
            },
        ) in traits.enumerate()
        {
            if i != 0 {
                ret.extend(quote!(+));
            }

            let (hrtb, life_use) = if life_use.is_empty() {
                (quote!(), quote!())
            } else {
                (quote!(for<'cglue_c>), quote!('cglue_c,))
            };

            ret.extend(quote!(#hrtb #path #raw_ident <#life_use #gen_use>));
        }

        ret
    }

    /// Optional and vtable definitions.
    ///
    /// Optional means they are of type `Option<&'cglue_a VTable>`.
    fn optional_vtbl_defs(&self, inst_ident: TokenStream, ctx_ident: TokenStream) -> TokenStream {
        let mut ret = TokenStream::new();

        let cont_name = &self.cont_name;

        let gen_all_use = &self.generics.gen_use;

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in &self.optional_vtbl
        {
            ret.extend(
                quote!(#vtbl_name: ::core::option::Option<&'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<#inst_ident, #ctx_ident, #gen_all_use>, #gen_use #assoc_use>>, ),
            );
        }

        ret
    }

    fn ret_tmp_defs<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            ret_tmp_name,
            path,
            ret_tmp_typename,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in self.mandatory_vtbl.iter().chain(iter)
        {
            ret.extend(
                quote!(#ret_tmp_name: #path #ret_tmp_typename<CGlueCtx, #gen_use #assoc_use>, ),
            );
        }

        ret
    }

    /// Mixed vtable definitoins.
    ///
    /// This function goes through optional vtables, and mixes them between `Option`, and
    /// non-`Option` types for the definitions.
    ///
    /// # Arguments
    ///
    /// * `iter` - iterator of required/mandatory types. These types will have non-`Option` type
    /// assigned. It is crucial to have the same order of values!
    fn mixed_opt_vtbl_defs<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        let mut iter = iter.peekable();

        let cont_name = &self.cont_name;

        let all_gen_use = &self.generics.gen_use;

        for (
            TraitInfo {
                vtbl_name,
                path,
                vtbl_typename,
                generics: ParsedGenerics { gen_use, .. },
                assocs:
                    ParsedGenerics {
                        gen_use: assoc_use, ..
                    },
                ..
            },
            mandatory,
        ) in self.optional_vtbl.iter().map(|v| {
            if iter.peek() == Some(&v) {
                iter.next();
                (v, true)
            } else {
                (v, false)
            }
        }) {
            let def = match mandatory {
                true => {
                    quote!(#vtbl_name: &'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use>, )
                }
                false => {
                    quote!(#vtbl_name: ::core::option::Option<&'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use>>, )
                }
            };
            ret.extend(def);
        }

        ret
    }

    /// Generate a `GetContainer` implementation for a specific cglue object.
    fn get_container_impl(
        &self,
        name: &Ident,
        trg_path: &TokenStream,
        all_generics: &ParsedGenerics,
    ) -> TokenStream {
        let cont_name = &self.cont_name;

        let ParsedGenerics {
            gen_declare,
            gen_use,
            gen_where_bounds,
            ..
        } = &all_generics;

        let ctx_bound = super::traits::ctx_bound();

        quote! {
            impl<CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #gen_declare>
                #trg_path::GetContainer for #name<'_, CGlueInst, CGlueCtx, #gen_use>
            where
                #cont_name<CGlueInst, CGlueCtx, #gen_use>: #trg_path::CGlueObjBase,
                #gen_where_bounds
            {
                type ContType = #cont_name<CGlueInst, CGlueCtx, #gen_use>;

                fn ccont_ref(&self) -> &Self::ContType {
                    &self.container
                }

                fn ccont_mut(&mut self) -> &mut Self::ContType {
                    &mut self.container
                }

                fn into_ccont(self) -> Self::ContType {
                    self.container
                }

                fn build_with_ccont(&self, container: Self::ContType) -> Self {
                    Self {
                        container,
                        ..*self
                    }
                }
            }
        }
    }

    fn cglue_obj_impl(&self, trg_path: &TokenStream, all_generics: &ParsedGenerics) -> TokenStream {
        let cont_name = &self.cont_name;

        let ParsedGenerics {
            gen_declare: all_gen_declare,
            gen_use: all_gen_use,
            gen_where_bounds: all_gen_where_bounds,
            ..
        } = &all_generics;

        let ctx_bound = super::traits::ctx_bound();

        let mut ret = quote! {
            impl<CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #all_gen_declare> #trg_path::CGlueObjBase
                for #cont_name<CGlueInst, CGlueCtx, #all_gen_use>
            where
                CGlueInst::Target: Sized,
                #all_gen_where_bounds
            {
                type ObjType = CGlueInst::Target;
                type InstType = CGlueInst;
                type Context = CGlueCtx;

                fn cobj_base_ref(&self) -> (&Self::ObjType, &Self::Context) {
                    (self.instance.deref(), &self.context)
                }

                fn cobj_base_owned(self) -> (Self::InstType, Self::Context) {
                    (self.instance, self.context)
                }
            }
        };

        for TraitInfo {
            path,
            ret_tmp_typename,
            ret_tmp_name,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in self.mandatory_vtbl.iter().chain(self.optional_vtbl.iter())
        {
            ret.extend(quote!{
                impl<CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #all_gen_declare>
                    #trg_path::CGlueObjRef<#path #ret_tmp_typename<CGlueCtx, #gen_use #assoc_use>>
                    for #cont_name<CGlueInst, CGlueCtx, #all_gen_use>
                where
                    CGlueInst::Target: Sized,
                    #all_gen_where_bounds
                {
                    fn cobj_ref(&self) -> (&Self::ObjType, &#path #ret_tmp_typename<CGlueCtx, #gen_use #assoc_use>, &Self::Context) {
                        (self.instance.deref(), &self.#ret_tmp_name, &self.context)
                    }
                }

                impl<
                        CGlueInst: ::core::ops::DerefMut,
                        CGlueCtx: #ctx_bound,
                        #all_gen_declare
                    > #trg_path::CGlueObjMut<#path #ret_tmp_typename<CGlueCtx, #gen_use #assoc_use>>
                    for #cont_name<CGlueInst, CGlueCtx, #all_gen_use>
                where
                    CGlueInst::Target: Sized,
                    #all_gen_where_bounds
                {
                    fn cobj_mut(&mut self) -> (&mut Self::ObjType, &mut #path #ret_tmp_typename<CGlueCtx, #gen_use #assoc_use>, &Self::Context) {
                        (
                            self.instance.deref_mut(),
                            &mut self.#ret_tmp_name,
                            &self.context,
                        )
                    }
                }
            });
        }

        ret
    }

    /// `GetVtbl<Vtable>`, `CGlueObjRef<RetTmp>`, `CGlueObjOwned<RetTmp>`, `CGlueObjBuild<RetTmp>`, and `CGlueObjMut<T, RetTmp>` implementations for mandatory vtables.
    fn mandatory_as_ref_impls(&self, trg_path: &TokenStream) -> TokenStream {
        self.as_ref_impls(
            &self.name,
            self.mandatory_vtbl.iter(),
            &self.generics,
            trg_path,
        )
    }

    /// `GetVtbl<Vtable>`, `CGlueObjRef<RetTmp>`, `CGlueObjOwned<RetTmp>`, `CGlueObjBuild<RetTmp>`, and `CGlueObjMut<T, RetTmp>` implementations for arbitrary type and list of tables.
    ///
    /// # Arguments
    ///
    /// * `name` - type name to implement the conversion for.
    /// * `traits` - vtable types to implement the conversion to.
    fn as_ref_impls<'a>(
        &'a self,
        name: &Ident,
        traits: impl Iterator<Item = &'a TraitInfo>,
        all_generics: &ParsedGenerics,
        trg_path: &TokenStream,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        let cont_name = &self.cont_name;

        let all_gen_declare = &all_generics.gen_declare;
        let all_gen_use = &all_generics.gen_use;
        let all_gen_where_bounds = &all_generics.gen_where_bounds;

        let ctx_bound = super::traits::ctx_bound();

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            vtbl_get_ident,
            assoc_bind_ident,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in traits
        {
            ret.extend(quote! {

                // TODO: bring back CGlueObjBuild

                impl<'cglue_a, CGlueInst, CGlueCtx: #ctx_bound, #all_gen_declare> #trg_path::GetVtblBase<#path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use>>
                    for #name<'cglue_a, CGlueInst, CGlueCtx, #all_gen_use>
                where
                    #cont_name<CGlueInst, CGlueCtx, #all_gen_use>: #trg_path::CGlueObjBase,
                    #all_gen_where_bounds
                {
                    fn get_vtbl_base(&self) -> &#path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use> {
                        &self.#vtbl_name
                    }
                }

                impl<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #all_gen_declare> #path #vtbl_get_ident<'cglue_a, #gen_use #assoc_use>
                    for #name<'cglue_a, CGlueInst, CGlueCtx, #all_gen_use>
                where
                    <CGlueInst as ::core::ops::Deref>::Target: Sized,
                    #cont_name<CGlueInst, CGlueCtx, #all_gen_use>: #trg_path::CGlueObjBase,
                    #all_gen_where_bounds
                {
                    fn get_vtbl(&self) -> &#path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #all_gen_use>, #gen_use #assoc_use> {
                        &self.#vtbl_name
                    }
                }

                impl<'cglue_a, CGlueInst: ::core::ops::Deref, CGlueCtx: #ctx_bound, #all_gen_declare> #path #assoc_bind_ident<#gen_use>
                    for #name<'cglue_a, CGlueInst, CGlueCtx, #all_gen_use>
                where
                    <CGlueInst as ::core::ops::Deref>::Target: Sized,
                    #cont_name<CGlueInst, CGlueCtx, #all_gen_use>: #trg_path::CGlueObjBase,
                    #all_gen_where_bounds
                {
                    type Assocs = (#assoc_use);
                }
            });
        }

        ret
    }

    /// List of `vtbl: Default::default(), ` for all mandatory vtables.
    fn mandatory_vtbl_defaults(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in &self.mandatory_vtbl {
            ret.extend(quote!(#vtbl_name: Default::default(),));
        }

        ret
    }

    fn mandatory_ret_tmp_defaults(&self) -> TokenStream {
        Self::ret_tmp_defaults(self.mandatory_vtbl.iter())
    }

    fn ret_tmp_defaults<'a>(iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { ret_tmp_name, .. } in iter {
            ret.extend(quote!(#ret_tmp_name: Default::default(),));
        }

        ret
    }

    /// List of `vtbl: None, ` for all optional vtables.
    #[cfg_attr(not(feature = "unstable"), allow(unused))]
    fn default_opt_vtbl_list(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        #[cfg(feature = "unstable")]
        let crate_path = crate::util::crate_path();

        let cont_name = &self.cont_name;

        let gen_all_use = &self.generics.gen_use;

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in &self.optional_vtbl
        {
            #[cfg(feature = "unstable")]
            {
                let vtbl_ty = quote!(&'cglue_a #path #vtbl_typename<'cglue_a, #cont_name<CGlueInst, CGlueCtx, #gen_all_use>, #gen_use #assoc_use>);
                ret.extend(quote!(#vtbl_name: <#vtbl_ty as #crate_path::TryDefault<#vtbl_ty>>::try_default(),));
            }
            #[cfg(not(feature = "unstable"))]
            ret.extend(quote!(#vtbl_name: None,));
        }

        ret
    }

    /// Simple identifier list.
    fn vtbl_list<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in iter {
            ret.extend(quote!(#vtbl_name,));
        }

        ret
    }

    /// Try-unwrapping assignment list `vtbl: vtbl?, `.
    ///
    /// # Arguments
    ///
    /// * `iter` - vtable identifiers to list and try-unwrap.
    fn vtbl_unwrap_list<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in iter {
            ret.extend(quote!(#vtbl_name: #vtbl_name?,));
        }

        ret
    }

    /// Mixed try-unwrap list for vtables.
    ///
    /// This function goes through optional vtables, unwraps the ones in `iter`, leaves others
    /// bare.
    ///
    /// # Arguments
    ///
    /// * `iter` - list of vtables to try-unwrap. Must be ordered the same way!
    fn mixed_opt_vtbl_unwrap_list<'a>(
        &'a self,
        iter: impl Iterator<Item = &'a TraitInfo>,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        let mut iter = iter.peekable();

        for (TraitInfo { vtbl_name, .. }, mandatory) in self.optional_vtbl.iter().map(|v| {
            if iter.peek() == Some(&v) {
                iter.next();
                (v, true)
            } else {
                (v, false)
            }
        }) {
            let def = match mandatory {
                true => quote!(#vtbl_name: #vtbl_name?, ),
                false => quote!(#vtbl_name, ),
            };
            ret.extend(def);
        }

        ret
    }

    /// Try-unwrap a list of vtables without assigning them (`vtbl?,`).
    ///
    /// # Arguments
    ///
    /// * `iter` - vtables to unwrap.
    fn vtbl_unwrap_validate<'a>(
        &'a self,
        iter: impl Iterator<Item = &'a TraitInfo>,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in iter {
            ret.extend(quote!((*#vtbl_name)?,));
        }

        ret
    }

    /// Bind `Default` to mandatory vtables.
    pub fn vtbl_where_bounds<'a>(
        iter: impl Iterator<Item = &'a TraitInfo>,
        cont_name: &Ident,
        container_ident: TokenStream,
        ctx_ident: TokenStream,
        all_generics: &ParsedGenerics,
        trait_bound: Option<&TokenStream>,
        vtbl_lifetime: &Lifetime,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        let all_gen_use = &all_generics.gen_use;

        for TraitInfo {
            path,
            raw_ident,
            vtbl_typename,
            generics: ParsedGenerics {
                gen_use, life_use, ..
            },
            assocs: ParsedGenerics {
                gen_use: assoc_use, ..
            },
            ..
        } in iter
        {
            // FIXME: this is a bit of a hack. 0.1 could do multiple generic implementations
            // without trait bounds just fine.
            if let Some(trait_bound) = &trait_bound {
                // FIXME: this will not work with multiple lifetimes.
                let life_use = if life_use.is_empty() {
                    None
                } else {
                    Some(quote!('cglue_a,))
                };

                ret.extend(quote!(#trait_bound: #path #raw_ident<#life_use #gen_use>,));
            }

            ret.extend(quote!(&#vtbl_lifetime #path #vtbl_typename<#vtbl_lifetime, #cont_name<#container_ident, #ctx_ident, #all_gen_use>, #gen_use #assoc_use>: #vtbl_lifetime + Default,));
        }

        ret
    }
}
