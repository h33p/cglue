use crate::util::*;
use itertools::*;
use proc_macro2::TokenStream;
use quote::*;
use syn::parse::{Parse, ParseStream};
use syn::*;

/// Describes information about a single trait.
#[derive(PartialEq, Eq, Ord, PartialOrd)]
pub struct TraitInfo {
    ident: Ident,
    vtbl_name: Ident,
    lc_name: Ident,
    vtbl_typename: Ident,
}

impl From<Ident> for TraitInfo {
    fn from(ident: Ident) -> Self {
        Self {
            vtbl_name: format_ident!("vtbl_{}", ident.to_string().to_lowercase()),
            lc_name: format_ident!("{}", ident.to_string().to_lowercase()),
            vtbl_typename: format_ident!("CGlueVtbl{}", ident),
            ident,
        }
    }
}

/// Describes parse trait group, allows to generate code for it.
pub struct TraitGroup {
    name: Ident,
    mandatory_vtbl: Vec<TraitInfo>,
    optional_vtbl: Vec<TraitInfo>,
}

impl Parse for TraitGroup {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;

        input.parse::<Token![,]>()?;
        let mandatory_traits = parse_maybe_braced_idents(input)?;

        input.parse::<Token![,]>()?;
        let optional_traits = parse_maybe_braced_idents(input)?;

        let mut mandatory_vtbl: Vec<TraitInfo> =
            mandatory_traits.into_iter().map(TraitInfo::from).collect();
        mandatory_vtbl.sort();

        let mut optional_vtbl: Vec<TraitInfo> =
            optional_traits.into_iter().map(TraitInfo::from).collect();
        optional_vtbl.sort();

        Ok(Self {
            name,
            mandatory_vtbl,
            optional_vtbl,
        })
    }
}

/// Describes trait group to be implemented on a type.
pub struct TraitGroupImpl {
    name: Ident,
    group: Ident,
    implemented_vtbl: Vec<TraitInfo>,
}

impl Parse for TraitGroupImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![,]>()?;

        let group = input.parse()?;

        input.parse::<Token![,]>()?;
        let implemented_traits = parse_maybe_braced_idents(input)?;

        let mut implemented_vtbl: Vec<TraitInfo> =
            implemented_traits.into_iter().map(From::from).collect();

        implemented_vtbl.sort();

        Ok(Self {
            name,
            group,
            implemented_vtbl,
        })
    }
}

impl TraitGroupImpl {
    /// Generate trait group conversion for a specific type.
    ///
    /// The type will have specified vtables implemented as a conversion function.
    pub fn implement_group(&self) -> TokenStream {
        let name = &self.name;
        let group = &self.group;

        let filler_trait = format_ident!("{}VtableFiller", group);
        let vtable_type = format_ident!("{}Vtables", group);

        let implemented_tables = TraitGroup::some_default_vtbl_defs(self.implemented_vtbl.iter());

        quote! {
            impl<'a> #filler_trait<'a> for #name {
                fn fill_table(table: #vtable_type<'a, Self>) -> #vtable_type<'a, Self> {
                    #vtable_type {
                        #implemented_tables
                        ..table
                    }
                }
            }
        }
    }
}

pub struct TraitCastGroup {
    name: Ident,
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
        let name = input.parse()?;

        let implemented_traits = input.parse::<TypeImplTrait>()?;

        let mut needed_vtbls: Vec<TraitInfo> = implemented_traits
            .bounds
            .into_iter()
            .map(|b| format_ident!("{}", b.to_token_stream().to_string()))
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

    pub fn optional_func_name_with_mand<'a>(
        &'a self,
        prefix: &str,
        lc_names: impl Iterator<Item = &'a TraitInfo>,
    ) -> Ident {
        let mut lc_names = self.mandatory_vtbl.iter().chain(lc_names).collect_vec();
        lc_names.sort();
        Self::optional_func_name(prefix, lc_names.into_iter())
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

    /// Generate full code for the trait group.
    ///
    /// This trait group will have all variants generated for converting, building, and
    /// converting it.
    pub fn create_group(&self) -> TokenStream {
        // Path to trait group import.
        let crate_path = crate::util::crate_path();
        let trg_path: TokenStream = quote!(#crate_path::trait_group);

        let c_void = quote!(::core::ffi::c_void);

        let name = &self.name;

        let mandatory_vtbl_defs = self.mandatory_vtbl_defs(self.mandatory_vtbl.iter());
        let optional_vtbl_defs = self.optional_vtbl_defs();

        let mandatory_as_ref_impls = self.mandatory_as_ref_impls();
        let mand_vtbl_default = self.mandatory_vtbl_defaults();
        let none_opt_vtbl_list = self.none_opt_vtbl_list();
        let mand_vtbl_list = self.vtbl_list(self.mandatory_vtbl.iter());
        let full_opt_vtbl_list = self.vtbl_list(self.optional_vtbl.iter());
        let vtbl_where_bounds = self.vtbl_where_bounds(self.mandatory_vtbl.iter());

        let mut trait_funcs = TokenStream::new();

        let mut opt_structs = TokenStream::new();

        let impl_traits =
            self.impl_traits(self.mandatory_vtbl.iter().chain(self.optional_vtbl.iter()));
        let base_doc = format!(
            "Trait group potentially implementing `{}` traits.",
            impl_traits
        );
        let trback_doc = format!("be transformed back into `{}` without losing data.", name);
        let new_doc = format!("Create new instance of {}.", name);

        let opaque_name = format_ident!("{}Opaque", name);
        let opaque_name_ref = format_ident!("{}OpaqueRef", name);
        let opaque_name_mut = format_ident!("{}OpaqueMut", name);
        let opaque_name_boxed = format_ident!("{}OpaqueBox", name);

        let filler_trait = format_ident!("{}VtableFiller", name);
        let vtable_type = format_ident!("{}Vtables", name);

        for traits in self
            .optional_vtbl
            .iter()
            .powerset()
            .filter(|v| !v.is_empty())
        {
            let func_name = Self::optional_func_name("cast", traits.iter().copied());
            let func_name_with_mand =
                self.optional_func_name_with_mand("cast", traits.iter().copied());
            let func_name_final = Self::optional_func_name("into", traits.iter().copied());
            let func_name_final_with_mand =
                self.optional_func_name_with_mand("into", traits.iter().copied());
            let func_name_check = Self::optional_func_name("check", traits.iter().copied());
            let func_name_check_with_mand =
                self.optional_func_name_with_mand("check", traits.iter().copied());
            let func_name_mut = Self::optional_func_name("as_mut", traits.iter().copied());
            let func_name_mut_with_mand =
                self.optional_func_name_with_mand("as_mut", traits.iter().copied());
            let func_name_ref = Self::optional_func_name("as_ref", traits.iter().copied());
            let func_name_ref_with_mand =
                self.optional_func_name_with_mand("as_ref", traits.iter().copied());
            let opt_final_name = Self::optional_group_ident(&name, "Final", traits.iter().copied());
            let opt_name = Self::optional_group_ident(&name, "", traits.iter().copied());
            let opt_vtbl_defs = self.mandatory_vtbl_defs(traits.iter().copied());
            let opt_mixed_vtbl_defs = self.mixed_opt_vtbl_defs(traits.iter().copied());

            let opt_as_ref_impls = self.as_ref_impls(
                &opt_name,
                self.mandatory_vtbl.iter().chain(traits.iter().copied()),
            );

            let opt_vtbl_list = self.vtbl_list(traits.iter().copied());
            let opt_vtbl_unwrap = self.vtbl_unwrap_list(traits.iter().copied());
            let opt_vtbl_unwrap_validate = self.vtbl_unwrap_validate(traits.iter().copied());

            let mixed_opt_vtbl_unwrap = self.mixed_opt_vtbl_unwrap_list(traits.iter().copied());

            let impl_traits =
                self.impl_traits(self.mandatory_vtbl.iter().chain(traits.iter().copied()));

            let transmuter_type = format_ident!("CGlueTransmute{}", opt_name);

            let opt_final_doc = format!(
                "Final {} variant with `{}` implemented.",
                name, &impl_traits
            );
            let opt_final_doc2 = format!(
                "Retrieve this type using [`{}`]({}::{}) function.",
                func_name_final, name, func_name_final
            );

            let opt_doc = format!(
                "Concrete {} variant with `{}` implemented.",
                name, &impl_traits
            );
            let opt_doc2 = format!("Retrieve this type using one of [`{}`]({}::{}), [`{}`]({}::{}), or [`{}`]({}::{}) functions.", func_name, name, func_name, func_name_mut, name, func_name_mut, func_name_ref, name, func_name_ref);

            opt_structs.extend(quote! {

                // Final implementation - more compact layout.

                #[doc = #opt_final_doc]
                ///
                #[doc = #opt_final_doc2]
                #[repr(C)]
                pub struct #opt_final_name<'a, T, F> {
                    instance: T,
                    #mandatory_vtbl_defs
                    #opt_vtbl_defs
                }

                impl<T: ::core::ops::Deref<Target = F>, F>
                    #trg_path::CGlueObjRef<F> for #opt_final_name<'_, T, F>
                {
                    fn cobj_ref(&self) -> &F {
                        self.instance.deref()
                    }
                }

                impl<T: ::core::ops::Deref<Target = F> + ::core::ops::DerefMut, F>
                    #trg_path::CGlueObjMut<F> for #opt_final_name<'_, T, F>
                {
                    fn cobj_mut(&mut self) -> &mut F {
                        self.instance.deref_mut()
                    }
                }

                #opt_as_ref_impls

                // Non-final implementation. Has the same layout as the base struct.

                #[doc = #opt_doc]
                ///
                #[doc = #opt_doc2]
                #[repr(C)]
                pub struct #opt_name<'a, T, F> {
                    instance: T,
                    #mandatory_vtbl_defs
                    #opt_mixed_vtbl_defs
                }

                impl<T: ::core::ops::Deref<Target = F>, F>
                    #trg_path::CGlueObjRef<F> for #opt_name<'_, T, F>
                {
                    fn cobj_ref(&self) -> &F {
                        self.instance.deref()
                    }
                }

                impl<T: ::core::ops::Deref<Target = F> + ::core::ops::DerefMut, F>
                    #trg_path::CGlueObjMut<F> for #opt_name<'_, T, F>
                {
                    fn cobj_mut(&mut self) -> &mut F {
                        self.instance.deref_mut()
                    }
                }

                unsafe impl<'a, T, F> #trg_path::Opaquable for #opt_name<'a, T, F> {
                    type OpaqueTarget = #name<'a, T, F>;
                }

                impl<'a, T, F> From<#opt_name<'a, T, F>> for #name<'a, T, F> {
                    fn from(input: #opt_name<'a, T, F>) -> Self {
                        #trg_path::Opaquable::into_opaque(input)
                    }
                }
            });

            let func_final_doc1 = format!(
                "Retrieve a final {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_final_doc2 = format!(
                "This consumes the `{}`, and outputs `Some(impl {})`, if all types are present.",
                name, impl_traits
            );

            let func_doc1 = format!(
                "Retrieve a concrete {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_doc2 = format!("This consumes the `{}`, and outputs `Some(impl {})`, if all types are present. It is possible to cast this type back with the `From` implementation.", name, impl_traits);

            let func_check_doc1 = format!("Check whether {} implements `{}`.", name, impl_traits);
            let func_check_doc2 = format!(
                "If this check returns true, it is safe to run consuming conversion operations."
            );

            let func_mut_doc1 = format!(
                "Retrieve mutable reference to a concrete {} variant that implements `{}`.",
                name, impl_traits
            );
            let func_ref_doc1 = format!(
                "Retrieve immutable reference to a concrete {} variant that implements `{}`.",
                name, impl_traits
            );

            trait_funcs.extend(quote! {
                #[doc = #func_check_doc1]
                ///
                #[doc = #func_check_doc2]
                pub fn #func_name_check(&self) -> bool
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name_ref().is_some()
                }

                #[doc = #func_check_doc1]
                ///
                #[doc = #func_check_doc2]
                pub fn #func_name_check_with_mand(&self) -> bool
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name_check()
                }

                #[doc = #func_final_doc1]
                ///
                #[doc = #func_final_doc2]
                pub fn #func_name_final(self) -> ::core::option::Option<impl 'a + #impl_traits>
                    where #opt_final_name<'a, T, F>: 'a + #impl_traits
                {
                    let #name {
                        instance,
                        #mand_vtbl_list
                        #opt_vtbl_list
                        ..
                    } = self;

                    Some(#opt_final_name {
                        instance,
                        #mand_vtbl_list
                        #opt_vtbl_unwrap
                    })
                }

                #[doc = #func_final_doc1]
                ///
                #[doc = #func_final_doc2]
                pub fn #func_name_final_with_mand(self) -> ::core::option::Option<impl 'a + #impl_traits>
                    where #opt_final_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name_final()
                }

                #[doc = #func_doc1]
                ///
                #[doc = #func_doc2]
                pub fn #func_name(self) -> ::core::option::Option<#opt_name<'a, T, F>>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    let #name {
                        instance,
                        #mand_vtbl_list
                        #full_opt_vtbl_list
                    } = self;

                    Some(#opt_name {
                        instance,
                        #mand_vtbl_list
                        #mixed_opt_vtbl_unwrap
                    })
                }

                #[doc = #func_doc1]
                ///
                #[doc = #func_doc2]
                pub fn #func_name_with_mand(self) -> ::core::option::Option<#opt_name<'a, T, F>>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name()
                }

                #[doc = #func_mut_doc1]
                pub fn #func_name_mut<'b>(&'b mut self) -> ::core::option::Option<&'b mut (impl 'a + #impl_traits)>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
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
                        (self as *mut Self as *mut #opt_name<T, F>).as_mut()
                    }
                }

                #[doc = #func_mut_doc1]
                pub fn #func_name_mut_with_mand<'b>(&'b mut self) -> ::core::option::Option<&'b mut (impl 'a + #impl_traits)>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name_mut()
                }

                #[doc = #func_ref_doc1]
                pub fn #func_name_ref<'b>(&'b self) -> ::core::option::Option<&'b (impl 'a + #impl_traits)>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
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
                        (self as *const Self as *const #opt_name<T, F>).as_ref()
                    }
                }

                #[doc = #func_ref_doc1]
                pub fn #func_name_ref_with_mand<'b>(&'b self) -> ::core::option::Option<&'b (impl 'a + #impl_traits)>
                    where #opt_name<'a, T, F>: 'a + #impl_traits
                {
                    self.#func_name_ref()
                }
            });
        }

        quote! {
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
            pub struct #name<'a, T, F> {
                instance: T,
                #mandatory_vtbl_defs
                #optional_vtbl_defs
            }

            #[repr(C)]
            pub struct #vtable_type<'a, F> {
                #mandatory_vtbl_defs
                #optional_vtbl_defs
            }

            impl<'a, F> Default for #vtable_type<'a, F>
                where #vtbl_where_bounds
            {
                fn default() -> Self {
                    Self {
                        #mand_vtbl_default
                        #none_opt_vtbl_list
                    }
                }
            }

            pub trait #filler_trait<'a>: Sized {
                fn fill_table(table: #vtable_type<'a, Self>) -> #vtable_type<'a, Self>;
            }

            pub type #opaque_name<'a, T: ::core::ops::Deref<Target = #c_void>> = #name<'a, T, T::Target>;
            pub type #opaque_name_ref<'a> = #name<'a, &'a #c_void, #c_void>;
            pub type #opaque_name_mut<'a> = #name<'a, &'a mut #c_void, #c_void>;
            pub type #opaque_name_boxed<'a> = #name<'a, #crate_path::boxed::CBox<#c_void>, #c_void>;

            impl<'a, T: ::core::ops::Deref<Target = F>, F: #filler_trait<'a>> From<T> for #name<'a, T, F>
                where #vtbl_where_bounds
            {
                fn from(instance: T) -> Self {
                    let vtbl = #filler_trait::fill_table(Default::default());

                    let #vtable_type {
                        #mand_vtbl_list
                        #full_opt_vtbl_list
                    } = vtbl;

                    Self {
                        instance,
                        #mand_vtbl_list
                        #full_opt_vtbl_list
                    }
                }
            }

            impl<'a, F: #filler_trait<'a>> From<F> for #name<'a, #crate_path::boxed::CBox<F>, F>
                where #vtbl_where_bounds
            {
                fn from(instance: F) -> Self {
                    #name::from(#crate_path::boxed::CBox::from(instance))
                }
            }

            impl<'a, T: ::core::ops::Deref<Target = F>, F: 'a> #name<'a, T, F>

                where #vtbl_where_bounds
            {
                #[doc = #new_doc]
                pub fn new(instance: T, #optional_vtbl_defs) -> Self
                    where #vtbl_where_bounds
                {
                    Self {
                        instance,
                        #mand_vtbl_default
                        #full_opt_vtbl_list
                    }
                }
            }

            impl<'a, F> #name<'a, #crate_path::boxed::CBox<F>, F> {
                #[doc = #new_doc]
                ///
                /// `instance` will be moved onto heap.
                pub fn new_boxed(instance: F, #optional_vtbl_defs) -> Self
                    where #vtbl_where_bounds
                {
                    Self {
                        instance: From::from(instance),
                        #mand_vtbl_default
                        #full_opt_vtbl_list
                    }
                }
            }

            /// Convert into opaque object.
            ///
            /// This is the prerequisite for using underlying trait implementations.
            unsafe impl<'a, T: #trg_path::Opaquable + ::core::ops::Deref<Target = F>, F> #trg_path::Opaquable for #name<'a, T, F> {
                type OpaqueTarget = #name<'a, T::OpaqueTarget, #c_void>;
            }

            impl<'a, T, F> #name<'a, T, F> {
                #trait_funcs
            }

            impl<T: ::core::ops::Deref<Target = F>, F>
                #trg_path::CGlueObjRef<F> for #name<'_, T, F>
            {
                fn cobj_ref(&self) -> &F {
                    self.instance.deref()
                }
            }

            impl<T: ::core::ops::Deref<Target = F> + ::core::ops::DerefMut, F>
                #trg_path::CGlueObjMut<F> for #name<'_, T, F>
            {
                fn cobj_mut(&mut self) -> &mut F {
                    self.instance.deref_mut()
                }
            }

            #mandatory_as_ref_impls

            #opt_structs
        }
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
            vtbl_typename,
            ..
        } in iter
        {
            ret.extend(quote!(#vtbl_name: &'a #vtbl_typename<F>, ));
        }

        ret
    }

    /// Get a sequence of `Trait1 + Trait2 + Trait3 ...`
    ///
    /// # Arguments
    ///
    /// * `traits` - traits to combine.
    fn impl_traits<'a>(&'a self, mut traits: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let first = &traits.next().unwrap().ident;

        let mut ret = quote!(#first);

        for TraitInfo { ident, .. } in traits {
            ret.extend(quote!(+ #ident));
        }

        ret
    }

    /// Optional and vtable definitions.
    ///
    /// Optional means they are of type `Option<&'a VTable>`.
    fn optional_vtbl_defs(&self) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            vtbl_name,
            vtbl_typename,
            ..
        } in &self.optional_vtbl
        {
            ret.extend(quote!(#vtbl_name: ::core::option::Option<&'a #vtbl_typename<F>>, ));
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
                vtbl_typename,
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
                true => quote!(#vtbl_name: &'a #vtbl_typename<F>, ),
                false => quote!(#vtbl_name: ::core::option::Option<&'a #vtbl_typename<F>>, ),
            };
            ret.extend(def);
        }

        ret
    }

    pub fn some_default_vtbl_defs<'a>(iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_name, .. } in iter {
            ret.extend(quote!(#vtbl_name: Some(Default::default()),));
        }

        ret
    }

    /// `AsRef<Vtable>` implementations for mandatory vtables.
    fn mandatory_as_ref_impls(&self) -> TokenStream {
        self.as_ref_impls(&self.name, self.mandatory_vtbl.iter())
    }

    /// `AsRef<Vtable>` implementations for arbitrary type and list of tables.
    ///
    /// # Arguments
    ///
    /// * `name` - type name to implement the conversion for.
    /// * `traits` - vtable types to implement the conversion to.
    fn as_ref_impls<'a>(
        &'a self,
        name: &Ident,
        traits: impl Iterator<Item = &'a TraitInfo>,
    ) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo {
            vtbl_name,
            vtbl_typename,
            ..
        } in traits
        {
            ret.extend(quote! {
                impl<T, F> AsRef<#vtbl_typename<F>> for #name<'_, T, F>
                {
                    fn as_ref(&self) -> &#vtbl_typename<F> {
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
    fn vtbl_where_bounds<'a>(&'a self, iter: impl Iterator<Item = &'a TraitInfo>) -> TokenStream {
        let mut ret = TokenStream::new();

        for TraitInfo { vtbl_typename, .. } in iter {
            ret.extend(quote!(&'a #vtbl_typename<F>: Default,));
        }

        ret
    }
}
