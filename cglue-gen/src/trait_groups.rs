use super::ext::*;
use super::generics::ParsedGenerics;
use crate::util::*;
use itertools::*;
use proc_macro2::TokenStream;
use quote::*;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::*;

/// Describes information about a single trait.
pub struct TraitInfo {
    path: Path,
    ident: Ident,
    generics: ParsedGenerics,
    vtbl_name: Ident,
    ret_tmp_typename: Ident,
    ret_tmp_name: Ident,
    enable_vtbl_name: Ident,
    lc_name: Ident,
    vtbl_typename: Ident,
}

impl PartialEq for TraitInfo {
    fn eq(&self, o: &Self) -> bool {
        self.ident == o.ident
    }
}

impl Eq for TraitInfo {}

impl Ord for TraitInfo {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.ident.cmp(&o.ident)
    }
}

impl PartialOrd for TraitInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Path> for TraitInfo {
    fn from(in_path: Path) -> Self {
        let (path, ident, gens) =
            split_path_ident(&in_path).expect("Failed to split path by idents");

        let lc_ident = ident.to_string().to_lowercase();

        Self {
            vtbl_name: format_ident!("vtbl_{}", lc_ident),
            lc_name: format_ident!("{}", lc_ident),
            vtbl_typename: format_ident!("{}Vtbl", ident),
            ret_tmp_typename: format_ident!("{}RetTmp", ident),
            ret_tmp_name: format_ident!("ret_tmp_{}", lc_ident),
            enable_vtbl_name: format_ident!("enable_{}", ident.to_string().to_lowercase()),
            path,
            ident,
            generics: ParsedGenerics::from(gens.as_ref()),
        }
    }
}

/// Describes parse trait group, allows to generate code for it.
pub struct TraitGroup {
    name: Ident,
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
        group::parse_braces(input).ok();

        input.parse::<Token![,]>()?;
        let mandatory_traits = parse_maybe_braced::<Path>(input)?;

        input.parse::<Token![,]>()?;
        let optional_traits = parse_maybe_braced::<Path>(input)?;

        let ext_trait_defs = if input.parse::<Token![,]>().is_ok() {
            parse_maybe_braced::<ItemTrait>(input)?
        } else {
            vec![]
        };

        let mut ext_traits = HashMap::new();

        let mut mandatory_vtbl: Vec<TraitInfo> = mandatory_traits
            .into_iter()
            .map(prelude_remap)
            .map(TraitInfo::from)
            .collect();
        mandatory_vtbl.sort();

        let mut optional_vtbl: Vec<TraitInfo> = optional_traits
            .into_iter()
            .map(prelude_remap)
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
            if let Some(tr) = ext_trait_defs.iter().find(|tr| tr.ident == vtbl.ident) {
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
                    .get(&(vtbl.path.clone(), vtbl.ident.clone()))
                    .or_else(|| {
                        store_exports.get(&vtbl.ident).and_then(|p| {
                            vtbl.path = p.clone();
                            store_traits.get(&(p.clone(), vtbl.ident.clone()))
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
                        vtbl.ident
                    );
                }
            }
        }

        let extra_filler_traits = if input.parse::<Token![,]>().is_ok() {
            input.parse::<LitBool>()?.value
        } else {
            true
        };

        Ok(Self {
            name,
            generics,
            mandatory_vtbl,
            optional_vtbl,
            ext_traits,
            extra_filler_traits,
        })
    }
}

/// Describes trait group to be implemented on a type.
pub struct TraitGroupImpl {
    ty_path: Path,
    ty: Ident,
    ty_generics: ParsedGenerics,
    generics: ParsedGenerics,
    group_path: Path,
    group: Ident,
    implemented_vtbl: Vec<TraitInfo>,
    fwd_implemented_vtbl: Option<Vec<TraitInfo>>,
}

impl Parse for TraitGroupImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let path = input.parse()?;

        let (ty_path, ty, ty_gens) = split_path_ident(&path)?;

        let mut ty_generics = ParsedGenerics::from(ty_gens.as_ref());

        input.parse::<Token![,]>()?;

        let group = input.parse()?;

        let (group_path, group, gens) = split_path_ident(&group)?;

        let generics = ParsedGenerics::from(gens.as_ref());

        let mut generics = match input.parse::<ParsedGenerics>() {
            Ok(ParsedGenerics {
                gen_where_bounds, ..
            }) => {
                group::parse_braces(input).ok();
                ParsedGenerics {
                    gen_where_bounds,
                    ..generics
                }
            }
            _ => generics,
        };

        generics.merge_and_remap(&mut ty_generics);

        let implemented_vtbl = if input.parse::<Token![,]>().is_ok() {
            let implemented_traits = parse_maybe_braced::<Path>(input)?;

            let mut implemented_vtbl: Vec<TraitInfo> = implemented_traits
                .into_iter()
                .map(prelude_remap)
                .map(ext_abs_remap)
                .map(From::from)
                .collect();

            implemented_vtbl.sort();

            implemented_vtbl
        } else {
            vec![]
        };

        let fwd_implemented_vtbl = if input.parse::<Token![,]>().is_ok() {
            let implemented_traits = parse_maybe_braced::<Path>(input)?;

            let mut implemented_vtbl: Vec<TraitInfo> = implemented_traits
                .into_iter()
                .map(prelude_remap)
                .map(ext_abs_remap)
                .map(From::from)
                .collect();

            implemented_vtbl.sort();

            Some(implemented_vtbl)
        } else {
            None
        };

        Ok(Self {
            ty_path,
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

impl TraitGroupImpl {
    /// Generate trait group conversion for a specific type.
    ///
    /// The type will have specified vtables implemented as a conversion function.
    pub fn implement_group(&self) -> TokenStream {
        let crate_path = crate_path();

        let ctx_bound = super::traits::ctx_bound();
        let cglue_c_opaque_bound = super::traits::cglue_c_opaque_bound();

        let ty_path = &self.ty_path;
        let ty = &self.ty;

        let ParsedGenerics {
            life_use: ty_life_use,
            gen_use: ty_gen_use,
            ..
        } = &self.ty_generics;

        let group = &self.group;
        let group_path = &self.group_path;
        let ParsedGenerics { gen_use, .. } = &self.generics;

        let ParsedGenerics {
            gen_declare,
            gen_where_bounds,
            ..
        } = [&self.ty_generics, &self.generics]
            .iter()
            .copied()
            .collect();

        let filler_trait = format_ident!("{}VtableFiller", group);
        let vtable_type = format_ident!("{}Vtables", group);

        let full_ty = quote!(#ty_path #ty <#ty_life_use #ty_gen_use>);

        let implemented_tables = TraitGroup::enable_opt_vtbls(self.implemented_vtbl.iter());
        let vtbl_where_bounds = TraitGroup::vtbl_where_bounds(
            self.implemented_vtbl.iter(),
            quote!(CGlueT),
            quote!(#full_ty),
            quote!(CGlueC),
            quote!(CGlueD),
        );

        let gen = quote! {
            impl<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = #full_ty>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #group_path #filler_trait<'cglue_a, CGlueT, CGlueC, CGlueD, #gen_use> for #full_ty
            where #gen_where_bounds #vtbl_where_bounds #cglue_c_opaque_bound {
                fn fill_table(table: #group_path #vtable_type<'cglue_a, CGlueT, #full_ty, CGlueC, CGlueD, #gen_use>) -> #group_path #vtable_type<'cglue_a, CGlueT, #full_ty, CGlueC, CGlueD, #gen_use> {
                    table #implemented_tables
                }
            }
        };

        if let Some(fwd_vtbl) = &self.fwd_implemented_vtbl {
            let fwd_filler_trait = format_ident!("{}FwdVtableFiller", group);

            let fwd_ty = quote!(#crate_path::forward::Fwd<&'cglue_a mut #full_ty>);

            let implemented_tables = TraitGroup::enable_opt_vtbls(fwd_vtbl.iter());
            let vtbl_where_bounds = TraitGroup::vtbl_where_bounds(
                fwd_vtbl.iter(),
                quote!(CGlueT),
                quote!(#fwd_ty),
                quote!(CGlueC),
                quote!(CGlueD),
            );

            quote! {
                #gen

                impl<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = #fwd_ty>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #group_path #fwd_filler_trait<'cglue_a, CGlueT, CGlueC, CGlueD, #gen_use> for #full_ty
                    where #gen_where_bounds #vtbl_where_bounds #cglue_c_opaque_bound {
                        fn fill_fwd_table(table: #group_path #vtable_type<'cglue_a, CGlueT, #fwd_ty, CGlueC, CGlueD, #gen_use>) -> #group_path #vtable_type<'cglue_a, CGlueT, #fwd_ty, CGlueC, CGlueD, #gen_use> {
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

        if let Ok(ExprReference {
            mutability, expr, ..
        }) = input.parse::<ExprReference>()
        {
            name = quote!(&#mutability #expr);
        } else {
            name = input.parse::<Ident>()?.into_token_stream();
        }

        let implemented_traits = input.parse::<TypeImplTrait>()?;

        let mut needed_vtbls: Vec<TraitInfo> = implemented_traits
            .bounds
            .into_iter()
            .filter_map(|b| match b {
                TypeParamBound::Trait(tr) => Some(tr.path),
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

        for TraitInfo { ident, .. } in traits {
            all_traits.push_str(&ident.to_string());
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

        let c_void = quote!(::core::ffi::c_void);

        let name = &self.name;

        let ParsedGenerics {
            gen_declare,
            gen_use,
            gen_where_bounds,
            ..
        } = &self.generics;

        let mandatory_vtbl_defs = self.mandatory_vtbl_defs(self.mandatory_vtbl.iter());
        let optional_vtbl_defs =
            self.optional_vtbl_defs(quote!(CGlueT), quote!(CGlueC), quote!(CGlueD));
        let optional_vtbl_defs_boxed = self.optional_vtbl_defs(
            quote!(#crate_path::boxed::CBox<'cglue_a, CGlueF>),
            quote!(#crate_path::trait_group::NoContext),
            quote!(#crate_path::trait_group::NoContext),
        );

        let mand_vtbl_default = self.mandatory_vtbl_defaults();
        let mand_ret_tmp_default = self.mandatory_ret_tmp_defaults();
        let full_opt_ret_tmp_default = Self::ret_tmp_defaults(self.optional_vtbl.iter());
        let mand_ret_tmp_list = self.mandatory_ret_tmp_list();
        let full_opt_ret_tmp_list = Self::ret_tmp_list(self.optional_vtbl.iter());
        let none_opt_vtbl_list = self.none_opt_vtbl_list();
        let mand_vtbl_list = self.vtbl_list(self.mandatory_vtbl.iter());
        let full_opt_vtbl_list = self.vtbl_list(self.optional_vtbl.iter());
        let full_opt_vtbl_copy = self.vtbl_copy_list(self.optional_vtbl.iter());
        let mandatory_as_ref_impls =
            self.mandatory_as_ref_impls(&trg_path, &full_opt_vtbl_copy, &full_opt_ret_tmp_default);
        let mandatory_internal_trait_impls = self.internal_trait_impls(
            name,
            self.mandatory_vtbl.iter(),
            &self.generics,
            &crate_path,
        );
        let vtbl_where_bounds = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            quote!(CGlueT),
            quote!(CGlueF),
            quote!(CGlueC),
            quote!(CGlueD),
        );
        let vtbl_where_bounds_boxed = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            quote!(#crate_path::boxed::CBox<'cglue_a, CGlueF>),
            quote!(CGlueF),
            quote!(#crate_path::trait_group::NoContext),
            quote!(#crate_path::trait_group::NoContext),
        );
        let vtbl_where_bounds_ctxboxed = Self::vtbl_where_bounds(
            self.mandatory_vtbl.iter(),
            quote!(#crate_path::boxed::CtxBox<'cglue_a, CGlueF, CGlueC>),
            quote!(CGlueF),
            quote!(CGlueC),
            quote!(CGlueD),
        );
        let ret_tmp_defs = self.ret_tmp_defs(self.optional_vtbl.iter());

        let mut enable_funcs = TokenStream::new();

        for TraitInfo {
            enable_vtbl_name,
            vtbl_typename,
            vtbl_name,
            path,
            generics: ParsedGenerics { gen_use, .. },
            ..
        } in &self.optional_vtbl
        {
            enable_funcs.extend(quote! {
                pub fn #enable_vtbl_name (self) -> Self
                    where &'cglue_a #path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>: Default {
                    Self {
                        #vtbl_name: Some(Default::default()),
                        ..self
                    }
                }
            });
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

        let base_name_ref = format_ident!("{}BaseRef", name);
        let base_name_mut = format_ident!("{}BaseMut", name);
        let base_name_boxed = format_ident!("{}BaseBox", name);
        let base_name_arc_box = format_ident!("{}BaseArcBox", name);
        let base_name_no_ctx_box = format_ident!("{}BaseNoCtxBox", name);
        let base_name_ctx_box = format_ident!("{}BaseCtxBox", name);
        let opaque_name = format_ident!("{}Opaque", name);
        let opaque_name_ref = format_ident!("{}Ref", name);
        let opaque_name_mut = format_ident!("{}Mut", name);
        let opaque_name_boxed = format_ident!("{}Box", name);
        let opaque_name_arc_box = format_ident!("{}ArcBox", name);
        let opaque_name_no_ctx_box = format_ident!("{}NoCtxBox", name);
        let opaque_name_ctx_box = format_ident!("{}CtxBox", name);

        let filler_trait = format_ident!("{}VtableFiller", name);
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
            let opt_final_name = Self::optional_group_ident(&name, "Final", traits.iter().copied());
            let opt_name = Self::optional_group_ident(&name, "", traits.iter().copied());
            let opt_vtbl_defs = self.mandatory_vtbl_defs(traits.iter().copied());
            let opt_mixed_vtbl_defs = self.mixed_opt_vtbl_defs(traits.iter().copied());

            let opt_vtbl_list = self.vtbl_list(traits.iter().copied());
            let opt_vtbl_copy = self.vtbl_copy_list(traits.iter().copied());
            let opt_vtbl_unwrap = self.vtbl_unwrap_list(traits.iter().copied());
            let opt_vtbl_unwrap_validate = self.vtbl_unwrap_validate(traits.iter().copied());
            let opt_ret_tmp_list = Self::ret_tmp_list(traits.iter().copied());
            let opt_ret_tmp_defaults = Self::ret_tmp_defaults(traits.iter().copied());
            let opt_ret_tmp_defs = self.ret_tmp_defs(traits.iter().copied());

            let mixed_opt_vtbl_unwrap = self.mixed_opt_vtbl_unwrap_list(traits.iter().copied());

            let sub_generics = self.used_generics(traits.iter().copied());

            let opt_as_ref_impls = self.as_ref_impls(
                &opt_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &trg_path,
                &full_opt_vtbl_copy,
                &full_opt_ret_tmp_default,
            );

            let opt_internal_trait_impls = self.internal_trait_impls(
                &opt_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &self.generics,
                &crate_path,
            );

            let opt_final_as_ref_impls = self.as_ref_impls(
                &opt_final_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &sub_generics,
                &trg_path,
                &opt_vtbl_copy,
                &opt_ret_tmp_defaults,
            );

            let opt_final_internal_trait_impls = self.internal_trait_impls(
                &opt_final_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
                &sub_generics,
                &crate_path,
            );

            let ParsedGenerics {
                gen_use: sub_gen_use,
                gen_declare: sub_gen_declare,
                gen_where_bounds: sub_where_bounds,
                ..
            } = sub_generics;

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
                pub struct #opt_final_name<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #sub_gen_declare> where #sub_where_bounds {
                    instance: CGlueT,
                    #mandatory_vtbl_defs
                    #opt_vtbl_defs
                    #opt_ret_tmp_defs
                }

                #opt_final_as_ref_impls

                #opt_final_internal_trait_impls

                // Non-final implementation. Has the same layout as the base struct.

                #[doc = #opt_doc]
                ///
                #[doc = #opt_doc2]
                #[repr(C)]
                pub struct #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare> where #gen_where_bounds {
                    instance: CGlueT,
                    #mandatory_vtbl_defs
                    #opt_mixed_vtbl_defs
                    #ret_tmp_defs
                }

                unsafe impl<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare> #trg_path::Opaquable for #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where #gen_where_bounds
{
                    type OpaqueTarget = #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>;
                }

                impl<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare> From<#opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>> for #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where #gen_where_bounds {
                    fn from(input: #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>) -> Self {
                        #trg_path::Opaquable::into_opaque(input)
                    }
                }

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
                    where #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>: 'cglue_a + #impl_traits
                {
                    self.#func_name_ref().is_some()
                }

                #[doc = #func_final_doc1]
                ///
                #[doc = #func_final_doc2]
                pub fn #func_name_final(self) -> ::core::option::Option<impl 'cglue_a + #impl_traits>
                    where #opt_final_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #sub_gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        instance,
                        #mand_vtbl_list
                        #opt_vtbl_list
                        #mand_ret_tmp_list
                        #opt_ret_tmp_list
                        ..
                    } = self;

                    Some(#opt_final_name {
                        instance,
                        #mand_vtbl_list
                        #opt_vtbl_unwrap
                        #mand_ret_tmp_list
                        #opt_ret_tmp_list
                    })
                }

                #[doc = #func_doc1]
                ///
                #[doc = #func_doc2]
                pub fn #func_name(self) -> ::core::option::Option<#opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>>
                    where #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        instance,
                        #mand_vtbl_list
                        #full_opt_vtbl_list
                        #mand_ret_tmp_list
                        #full_opt_ret_tmp_list
                    } = self;

                    Some(#opt_name {
                        instance,
                        #mand_vtbl_list
                        #mixed_opt_vtbl_unwrap
                        #mand_ret_tmp_list
                        #full_opt_ret_tmp_list
                    })
                }

                #[doc = #func_mut_doc1]
                pub fn #func_name_mut<'b>(&'b mut self) -> ::core::option::Option<&'b mut (impl 'cglue_a + #impl_traits)>
                    where #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        instance,
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
                        (self as *mut Self as *mut #opt_name<CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>).as_mut()
                    }
                }

                #[doc = #func_ref_doc1]
                pub fn #func_name_ref<'b>(&'b self) -> ::core::option::Option<&'b (impl 'cglue_a + #impl_traits)>
                    where #opt_name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>: 'cglue_a + #impl_traits
                {
                    let #name {
                        instance,
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
                        (self as *const Self as *const #opt_name<CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>).as_ref()
                    }
                }
            });
        }

        let (extra_filler_traits, extra_filler_trait_imports) = if self.extra_filler_traits {
            let traits = quote! {
                pub trait #fwd_filler_trait<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = #crate_path::forward::Fwd<&'cglue_a mut Self>>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare>: 'cglue_a + Sized where #gen_where_bounds {
                    fn fill_fwd_table(table: #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use>) -> #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use>;
                }

                impl<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = #crate_path::forward::Fwd<&'cglue_a mut CGlueF>>, CGlueF: #fwd_filler_trait<'cglue_a, CGlueT, CGlueC, CGlueD, #gen_use>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare>
                    #filler_trait<'cglue_a, CGlueT, CGlueC, CGlueD, #gen_use> for #crate_path::forward::Fwd<&'cglue_a mut CGlueF>
                    where #gen_where_bounds
                {
                    fn fill_table(table: #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use>) -> #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use> {
                        CGlueF::fill_fwd_table(table)
                    }
                }
            };

            let imports = quote! {
                #fwd_filler_trait,
            };

            (traits, imports)
        } else {
            (quote!(), quote!())
        };

        let submod_name = format_ident!("cglue_{}", name.to_string().to_lowercase());

        quote! {

            pub use #submod_name::{
                #name,
                #vtable_type,
                #filler_trait,
                #extra_filler_trait_imports
                #base_name_ref,
                #base_name_mut,
                #base_name_boxed,
                #base_name_arc_box,
                #base_name_no_ctx_box,
                #base_name_ctx_box,
                #opaque_name,
                #opaque_name_ref,
                #opaque_name_mut,
                #opaque_name_boxed,
                #opaque_name_arc_box,
                #opaque_name_no_ctx_box,
                #opaque_name_ctx_box,
                #opt_struct_imports
            };

            mod #submod_name {
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
                pub struct #name<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare> where #gen_where_bounds {
                    instance: CGlueT,
                    #mandatory_vtbl_defs
                    #optional_vtbl_defs
                    #ret_tmp_defs
                }

                #[repr(C)]
                pub struct #vtable_type<'cglue_a, CGlueT, CGlueF, CGlueC: 'static, CGlueD: 'static, #gen_declare> where #gen_where_bounds {
                    #mandatory_vtbl_defs
                    #optional_vtbl_defs
                }

                impl<'cglue_a, CGlueT, CGlueF, CGlueC: 'static + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static, #gen_declare> Default for #vtable_type<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #vtbl_where_bounds #gen_where_bounds
                {
                    fn default() -> Self {
                        Self {
                            #mand_vtbl_default
                            #none_opt_vtbl_list
                        }
                    }
                }

                impl<'cglue_a, CGlueT: ::core::ops::Deref<Target = CGlueF>, CGlueF, CGlueC: 'static + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #gen_where_bounds
                {
                    #enable_funcs
                }

                impl<'cglue_a, CGlueT: ::core::ops::Deref<Target = CGlueF>, CGlueF, CGlueC: 'static + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #vtable_type<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #gen_where_bounds
                {
                    #enable_funcs
                }

                pub trait #filler_trait<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = Self>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare>: Sized where #gen_where_bounds {
                    fn fill_table(table: #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use>) -> #vtable_type<'cglue_a, CGlueT, CGlueT::Target, CGlueC, CGlueD, #gen_use>;
                }

                #extra_filler_traits

                pub type #base_name_boxed<'cglue_a, CGlueF, #gen_use>
                    = #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueF>, CGlueF, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_ctx_box<'cglue_a, CGlueF, CGlueC, #gen_use>
                    = #name<'cglue_a, #crate_path::boxed::CtxBox<'cglue_a, CGlueF, CGlueC>, CGlueF, CGlueC, <CGlueC as #crate_path::trait_group::Opaquable>::OpaqueTarget, #gen_use>;

                pub type #base_name_no_ctx_box<'cglue_a, CGlueF, #gen_use>
                    = #base_name_ctx_box<'cglue_a, CGlueF, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_arc_box<'cglue_a, CGlueF, CGlueC, #gen_use>
                    = #base_name_ctx_box<'cglue_a, CGlueF, #crate_path::arc::COptArc<CGlueC>, #gen_use>;

                pub type #base_name_ref<'cglue_a, CGlueF, #gen_use>
                    = #name<'cglue_a, &'cglue_a CGlueF, CGlueF, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #base_name_mut<'cglue_a, CGlueF, #gen_use>
                    = #name<'cglue_a, &'cglue_a mut CGlueF, CGlueF, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>;

                pub type #opaque_name_boxed<'cglue_a, #gen_use>
                    = #base_name_boxed<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_ref<'cglue_a, #gen_use>
                    = #base_name_ref<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_mut<'cglue_a, #gen_use>
                    = #base_name_mut<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_ctx_box<'cglue_a, CGlueD, #gen_use>
                    = #base_name_ctx_box<'cglue_a, #c_void, CGlueD, #gen_use>;

                pub type #opaque_name_no_ctx_box<'cglue_a, #gen_use>
                    = #base_name_no_ctx_box<'cglue_a, #c_void, #gen_use>;

                pub type #opaque_name_arc_box<'cglue_a, #gen_use>
                    = #base_name_arc_box<'cglue_a, #c_void, #c_void, #gen_use>;

                pub type #opaque_name<'cglue_a, CGlueT: ::core::ops::Deref<Target = #c_void>, CGlueD, #gen_use>
                    = #name<'cglue_a, CGlueT, CGlueT::Target, CGlueD, CGlueD, #gen_use>;

                impl<'cglue_a, CGlueT: #ctx_bound ::core::ops::Deref<Target = CGlueF>, CGlueF: #filler_trait<'cglue_a, CGlueT, CGlueC, CGlueD, #gen_use>, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> From<CGlueT> for #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #vtbl_where_bounds #gen_where_bounds
                {
                    fn from(instance: CGlueT) -> Self {
                        let vtbl = #filler_trait::fill_table(Default::default());

                        let #vtable_type {
                            #mand_vtbl_list
                            #full_opt_vtbl_list
                        } = vtbl;

                        Self {
                            instance,
                            #mand_vtbl_list
                            #full_opt_vtbl_list
                            #mand_ret_tmp_default
                            #full_opt_ret_tmp_default
                        }
                    }
                }

                impl<'cglue_a, CGlueF, #gen_declare> From<CGlueF> for #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueF>, CGlueF, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>
                    where CGlueF: #filler_trait<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueF>, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>, #vtbl_where_bounds_boxed #gen_where_bounds
                {
                    fn from(instance: CGlueF) -> Self {
                        #name::from(#crate_path::boxed::CBox::from(instance))
                    }
                }

                impl<'cglue_a, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #gen_declare> From<(CGlueF, CGlueC)> for #name<'cglue_a, #crate_path::boxed::CtxBox<'cglue_a, CGlueF, CGlueC>, CGlueF, CGlueC, CGlueD, #gen_use>
                    where CGlueF: #filler_trait<'cglue_a, #crate_path::boxed::CtxBox<'cglue_a, CGlueF, CGlueC>, CGlueC, CGlueD, #gen_use>, #vtbl_where_bounds_ctxboxed #gen_where_bounds
                {
                    fn from((instance, ctx): (CGlueF, CGlueC)) -> Self {
                        #name::from(#crate_path::boxed::CtxBox::from((instance, ctx)))
                    }
                }

                impl<'cglue_a, CGlueT: ::core::ops::Deref<Target = CGlueF>, CGlueF: 'cglue_a, CGlueC: 'static + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #vtbl_where_bounds #gen_where_bounds
                {
                    #[doc = #new_doc]
                    pub fn new(instance: CGlueT, #optional_vtbl_defs) -> Self
                        where #vtbl_where_bounds
                    {
                        Self {
                            instance,
                            #mand_vtbl_default
                            #full_opt_vtbl_list
                            #mand_ret_tmp_default
                            #full_opt_ret_tmp_default
                        }
                    }
                }

                impl<'cglue_a, CGlueF, #gen_declare> #name<'cglue_a, #crate_path::boxed::CBox<'cglue_a, CGlueF>, CGlueF, #crate_path::trait_group::NoContext, #crate_path::trait_group::NoContext, #gen_use>
                    where #gen_where_bounds
                {
                    #[doc = #new_doc]
                    ///
                    /// `instance` will be moved onto heap.
                    pub fn new_boxed(instance: CGlueF, #optional_vtbl_defs_boxed) -> Self
                        where #vtbl_where_bounds_boxed
                    {
                        Self {
                            instance: From::from(instance),
                            #mand_vtbl_default
                            #full_opt_vtbl_list
                            #mand_ret_tmp_default
                            #full_opt_ret_tmp_default
                        }
                    }
                }

                /// Convert into opaque object.
                ///
                /// This is the prerequisite for using underlying trait implementations.
                unsafe impl<'cglue_a, CGlueT: #ctx_bound #trg_path::Opaquable + ::core::ops::Deref<Target = CGlueF>, CGlueF, CGlueC: 'static + Clone + #trg_path::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #gen_declare> #trg_path::Opaquable for #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #gen_where_bounds CGlueT::OpaqueTarget: #ctx_bound,
                {
                    type OpaqueTarget = #name<'cglue_a, CGlueT::OpaqueTarget, #c_void, CGlueD, CGlueD, #gen_use>;
                }

                impl<'cglue_a, CGlueT: #ctx_bound, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_declare> #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>
                    where #gen_where_bounds
                {
                    #trait_funcs
                }

                #mandatory_as_ref_impls

                #mandatory_internal_trait_impls

                #opt_structs
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

        let ctx_bound = super::traits::ctx_bound();

        let ParsedGenerics { gen_use, .. } = all_generics;

        for TraitInfo {
            path,
            ident,
            generics:
                ParsedGenerics {
                    life_use: tr_life_use,
                    gen_use: tr_gen_use,
                    ..
                },
            ..
        } in iter
        {
            if let Some((ext_path, tr_info)) = self.ext_traits.get(&ident) {
                let mut impls = TokenStream::new();

                let ext_name = format_ident!("{}Ext", ident);

                let (funcs, _, _) =
                    super::traits::parse_trait(tr_info, crate_path, super::traits::process_item);

                for func in &funcs {
                    func.int_trait_impl(Some(&ext_path), &ext_name, &mut impls);
                }

                let gen = quote! {
                    impl<'cglue_a, CGlueT: #ctx_bound, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #gen_use> #path #ident <#tr_life_use #tr_gen_use> for #self_ident<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> where Self: #ext_path #ext_name<#tr_life_use #tr_gen_use> {
                        #impls
                    }
                };

                ret.extend(gen);
            }
        }

        ret
    }

    /// Used generics by the vtable group.
    ///
    /// This will create a new instance of `ParsedGenerics` that will only contain used generic
    /// types.
    ///
    /// # Arguments
    ///
    /// * `iter` - iterator of types to use. This will be layered on top of mandatory types.
    fn used_generics<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> ParsedGenerics {
        self.generics.cross_ref(
            self.mandatory_vtbl
                .iter()
                .map(|i| &i.generics)
                .chain(iter.map(|i| &i.generics)),
        )
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

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            ..
        } in iter
        {
            ret.extend(
                quote!(#vtbl_name: &'cglue_a #path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>, ),
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
                ident,
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

            ret.extend(quote!(#hrtb #path #ident <#life_use #gen_use>));
        }

        ret
    }

    /// Optional and vtable definitions.
    ///
    /// Optional means they are of type `Option<&'cglue_a VTable>`.
    fn optional_vtbl_defs(
        &self,
        container_ident: TokenStream,
        ctx_ident: TokenStream,
        opaque_ctx_ident: TokenStream,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            ..
        } in &self.optional_vtbl
        {
            ret.extend(
                quote!(#vtbl_name: ::core::option::Option<&'cglue_a #path #vtbl_typename<'cglue_a, #container_ident, CGlueF, #ctx_ident, #opaque_ctx_ident, #gen_use>>, ),
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
            ..
        } in self.mandatory_vtbl.iter().chain(iter)
        {
            ret.extend(quote!(#ret_tmp_name: #path #ret_tmp_typename<#gen_use>, ));
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

        for (
            TraitInfo {
                vtbl_name,
                path,
                vtbl_typename,
                generics: ParsedGenerics { gen_use, .. },
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
                    quote!(#vtbl_name: &'cglue_a #path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>, )
                }
                false => {
                    quote!(#vtbl_name: ::core::option::Option<&'cglue_a #path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>>, )
                }
            };
            ret.extend(def);
        }

        ret
    }

    /// `GetVtbl<Vtable>`, `CGlueObjRef<RetTmp>`, `CGlueObjOwned<RetTmp>`, `CGlueObjBuild<RetTmp>`, and `CGlueObjMut<T, RetTmp>` implementations for mandatory vtables.
    fn mandatory_as_ref_impls(
        &self,
        trg_path: &TokenStream,
        opt_vtbl_copy: &TokenStream,
        opt_ret_tmp_default: &TokenStream,
    ) -> TokenStream {
        self.as_ref_impls(
            &self.name,
            self.mandatory_vtbl.iter(),
            &self.generics,
            trg_path,
            opt_vtbl_copy,
            opt_ret_tmp_default,
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
        opt_vtbl_copy: &TokenStream,
        opt_ret_tmp_default: &TokenStream,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        let crate_path = crate::util::crate_path();

        let all_gen_declare = &all_generics.gen_declare;
        let all_gen_use = &all_generics.gen_use;
        let all_gen_where_bounds = &all_generics.gen_where_bounds;

        let mand_vtbl_copy = self.vtbl_copy_list(self.mandatory_vtbl.iter());
        let mand_ret_tmp_default = self.mandatory_ret_tmp_defaults();

        for TraitInfo {
            vtbl_name,
            path,
            vtbl_typename,
            ret_tmp_typename,
            ret_tmp_name,
            generics: ParsedGenerics { gen_use, .. },
            ..
        } in traits
        {
            ret.extend(quote! {
                impl<CGlueT: ::core::ops::Deref<Target = CGlueF> + #crate_path::trait_group::ContextRef<Context = CGlueC, ObjType = CGlueF>, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #all_gen_declare>
                    #trg_path::CGlueObjRef<#path #ret_tmp_typename<#gen_use>> for #name<'_, CGlueT, CGlueF, CGlueC, CGlueD, #all_gen_use> where #all_gen_where_bounds
                {
                    type ObjType = CGlueF;
                    type ContType = CGlueT;
                    type Context = CGlueC;

                    fn cobj_ref(&self) -> (&CGlueF, &#path #ret_tmp_typename<#gen_use>, &CGlueC) {
                        let (obj, ctx) = self.instance.split_ctx_ref();
                        (obj, &self.#ret_tmp_name, ctx)
                    }
                }

                impl<CGlueT: ::core::ops::Deref<Target = CGlueF> + #crate_path::trait_group::ContextRef<Context = CGlueC, ObjType = CGlueF> + #crate_path::trait_group::ContextMut, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #all_gen_declare>
                    #trg_path::CGlueObjMut<#path #ret_tmp_typename<#gen_use>> for #name<'_, CGlueT, CGlueF, CGlueC, CGlueD, #all_gen_use> where #all_gen_where_bounds
                {
                    fn cobj_mut(&mut self) -> (&mut CGlueF, &mut #path #ret_tmp_typename<#gen_use>, &CGlueC) {
                        let (obj, ctx) = self.instance.split_ctx_mut();
                        (obj, &mut self.#ret_tmp_name, ctx)
                    }
                }

                impl<CGlueT: ::core::ops::Deref<Target = CGlueF> + ::core::ops::DerefMut + #crate_path::trait_group::ContextRef<Context = CGlueC, ObjType = CGlueF> + #crate_path::trait_group::ContextMut + #crate_path::trait_group::ContextOwned, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #all_gen_declare>
                    #trg_path::CGlueObjOwned<#path #ret_tmp_typename<#gen_use>> for #name<'_, CGlueT, CGlueF, CGlueC, CGlueD, #all_gen_use> where #all_gen_where_bounds
                {
                    fn cobj_owned(self) -> CGlueT {
                        self.instance
                    }
                }

                impl<CGlueT: ::core::ops::Deref<Target = CGlueF> + #crate_path::trait_group::ContextRef<ObjType = CGlueF, Context = CGlueC>, CGlueF, CGlueC: 'static + Clone + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD> + 'static + Clone, #all_gen_declare>
                    #trg_path::CGlueObjBuild<#path #ret_tmp_typename<#gen_use>> for #name<'_, CGlueT, CGlueF, CGlueC, CGlueD, #all_gen_use> where #all_gen_where_bounds
                {
                    unsafe fn cobj_build(&self, instance: Self::ContType) -> Self {
                        Self {
                            instance,
                            #mand_vtbl_copy
                            #opt_vtbl_copy
                            #mand_ret_tmp_default
                            #opt_ret_tmp_default
                        }
                    }
                }

                impl<'cglue_a, CGlueT, CGlueF, CGlueC: 'static + #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, CGlueD: #crate_path::trait_group::Opaquable<OpaqueTarget = CGlueD>, #all_gen_declare> #trg_path::GetVtbl<#path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use>>
                    for #name<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #all_gen_use>
                    where #all_gen_where_bounds
                {
                    fn get_vtbl(&self) -> &#path #vtbl_typename<'cglue_a, CGlueT, CGlueF, CGlueC, CGlueD, #gen_use> {
                        &self.#vtbl_name
                    }
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

    fn mandatory_ret_tmp_list(&self) -> TokenStream {
        Self::ret_tmp_list(self.mandatory_vtbl.iter())
    }

    fn ret_tmp_list<'a>(iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { ret_tmp_name, .. } in iter {
            ret.extend(quote!(#ret_tmp_name,));
        }

        ret
    }

    /// List of `vtbl: None, ` for all optional vtables.
    fn none_opt_vtbl_list(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in &self.optional_vtbl {
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

    /// Copy vtables from self
    fn vtbl_copy_list<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in iter {
            ret.extend(quote!(#vtbl_name: self.#vtbl_name,));
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
        container_ident: TokenStream,
        this_ident: TokenStream,
        ctx_ident: TokenStream,
        ctx_opaque_ident: TokenStream,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            path,
            vtbl_typename,
            generics: ParsedGenerics { gen_use, .. },
            ..
        } in iter
        {
            ret.extend(quote!(&'cglue_a #path #vtbl_typename<'cglue_a, #container_ident, #this_ident, #ctx_ident, #ctx_opaque_ident, #gen_use>: 'cglue_a + Default,));
        }

        ret
    }
}
