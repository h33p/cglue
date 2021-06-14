//! CGlue procedural macros

extern crate proc_macro;

use cglue_gen::arc_wrap::gen_wrap;
use cglue_gen::ext::{ext_abs_remap, prelude_remap_with_ident};
use cglue_gen::forward::gen_forward;
use cglue_gen::generics::{GenericCastType, GenericType};
use cglue_gen::trait_groups::*;
use proc_macro::TokenStream;
use quote::ToTokens;
use quote::{format_ident, quote};
use syn::*;

/// Make a trait CGlue compatible.
///
/// This macro will generate vtable structures alongside required traits and implementations needed
/// for constructing CGlue objects and their groups.
#[proc_macro_attribute]
pub fn cglue_trait(_args: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);

    let trait_def = cglue_gen::traits::gen_trait(tr, None);

    trait_def.into()
}

/// Make an external trait CGlue compatible.
///
/// Invoking this macro will change the name of the trait to be prefixed with `Ext`,
/// and it will act as a wrapper trait for the underlying trait.
///
/// This is very useful when third-party crates are needed to be CGlue compatible.
#[proc_macro_attribute]
pub fn cglue_trait_ext(_args: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);

    let ext_ident = format_ident!("Ext{}", tr.ident);

    let trait_def = cglue_gen::traits::gen_trait(tr, Some(&ext_ident));

    trait_def.into()
}

/// Convert into a CGlue compatible object.
///
/// The syntax is the same as a cast expression:
///
/// ```ignore
/// trait_obj!(variable as TraitName)
/// ```
///
/// It is possible to pass both owned objects and references.
#[proc_macro]
pub fn trait_obj(args: TokenStream) -> TokenStream {
    let crate_path = cglue_gen::util::crate_path();

    let GenericCastType {
        ident,
        target:
            GenericType {
                path,
                target,
                generics,
                ..
            },
    } = parse_macro_input!(args as GenericCastType);

    let path = if let Ok(ident) = parse2::<Ident>(target.clone()) {
        ext_abs_remap(prelude_remap_with_ident(path, &ident))
    } else {
        path
    };

    let target = format_ident!("CGlueBase{}", target.to_token_stream().to_string());

    let gen = quote! {
        #crate_path::trait_group::Opaquable::into_opaque({
            // We need rust to infer lifetimes and generics, thus we use a wrapper trait
            use #crate_path::from2::From2;
            #path #target :: <#generics>::from2(#ident)
        })
    };

    gen.into()
}

/// Define a CGlue trait group.
///
/// # Arguments
///
/// 1. The name of the group.
///
/// 2. Mandatory traits for the group. Either a single trait name, or a braced list of traits.
///
/// 3. Optionally implemented traits for the group. Either a single trait name, or a braced
///    list of traits.
///
/// 4. Optional block for external trait definitions. This block is needed when using non-standard
///    external traits.
#[proc_macro]
pub fn cglue_trait_group(args: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TraitGroup);
    args.create_group().into()
}

/// Implement a CGlue group for a specific type.
///
/// # Arguments
///
/// 1. The name of the type to implement the group for.
///
/// 2. The name of the group to implement.
///
/// 3. Optional traits that this object contains. Either a single trait, or a braced list of
///    traits.
#[proc_macro]
pub fn cglue_impl_group(args: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TraitGroupImpl);
    args.implement_group().into()
}

/// Convert into a CGlue trait group.
///
/// The syntax is the same as a cast expression:
///
/// ```ignore
/// group_obj!(variable as GroupName)
/// ```
///
/// It is possible to pass both owned objects and references.
#[proc_macro]
pub fn group_obj(args: TokenStream) -> TokenStream {
    let crate_path = cglue_gen::util::crate_path();

    let GenericCastType {
        ident,
        target:
            GenericType {
                path,
                target,
                generics,
                ..
            },
    } = parse_macro_input!(args as GenericCastType);

    let path = if let Ok(ident) = parse2::<Ident>(target.clone()) {
        ext_abs_remap(prelude_remap_with_ident(path, &ident))
    } else {
        path
    };

    let gen = quote! {
        #crate_path::trait_group::Opaquable::into_opaque({
            // We need rust to infer lifetimes and generics, thus we use a wrapper trait
            use #crate_path::from2::From2;
            #path #target :: <#generics>::from2(#ident)
        })
    };

    gen.into()
}

/// Checked cast to a list of optional traits.
///
/// The syntax is similar to a cast expression, but uses `impl` keyword:
///
/// ```ignore
/// cast!(obj impl Trait1 + Trait2 + Trait3);
/// ```
///
/// `cast!` is non-final, meaning it is possible to cast back to the base group object.
///
/// This macro accepts either:
///
/// 1. A list of optional traits, without any mandatory traits.
///
/// or
///
/// 2. A list of optional traits, with every mandatory trait.
///
/// In either case a successfully cast object will still implement the mandatory traits.
#[proc_macro]
pub fn cast(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as TraitCastGroup);
    cast.cast_group(CastType::Cast).into()
}

/// Checked cast to a list of optional traits.
///
/// The syntax is similar to a cast expression, but uses `impl` keyword:
///
/// ```ignore
/// as_ref!(obj impl Trait1 + Trait2 + Trait3);
/// ```
///
/// `as_ref!` is non-final, meaning once the reference is dropped, the original group object can be
/// used mutably.
///
/// This macro accepts either:
///
/// 1. A list of optional traits, without any mandatory traits.
///
/// or
///
/// 2. A list of optional traits, with every mandatory trait.
///
/// In either case a successfully cast object will still implement the mandatory traits.
#[proc_macro]
pub fn as_ref(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as TraitCastGroup);
    cast.cast_group(CastType::AsRef).into()
}

/// Checked cast to a list of optional traits.
///
/// The syntax is similar to a cast expression, but uses `impl` keyword:
///
/// ```ignore
/// as_mut!(obj impl Trait1 + Trait2 + Trait3);
/// ```
///
/// `as_mut!` is non-final, meaning once the reference is dropped, the original group object can be
/// used.
///
/// This macro accepts either:
///
/// 1. A list of optional traits, without any mandatory traits.
///
/// or
///
/// 2. A list of optional traits, with every mandatory trait.
///
/// In either case a successfully cast object will still implement the mandatory traits.
#[proc_macro]
pub fn as_mut(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as TraitCastGroup);
    cast.cast_group(CastType::AsMut).into()
}

/// Checked cast to a list of optional traits.
///
/// The syntax is similar to a cast expression, but uses `impl` keyword:
///
/// ```ignore
/// into!(obj impl Trait1 + Trait2 + Trait3);
/// ```
///
/// `into!` is final. After invoking this conversion it is not possible to retrieve the original
/// object.
///
/// This macro accepts either:
///
/// 1. A list of optional traits, without any mandatory traits.
///
/// or
///
/// 2. A list of optional traits, with every mandatory trait.
///
/// In either case a successfully cast object will still implement the mandatory traits.
#[proc_macro]
pub fn into(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as TraitCastGroup);
    cast.cast_group(CastType::Into).into()
}

/// Check if the group can be cast to the specified traits.
///
/// The syntax is similar to a cast expression, but uses `impl` keyword:
///
/// ```ignore
/// check!(obj impl Trait1 + Trait2 + Trait3);
/// ```
///
/// The result of `check!` will be a boolean value.
///
/// This macro accepts either:
///
/// 1. A list of optional traits, without any mandatory traits.
///
/// or
///
/// 2. A list of optional traits, with every mandatory trait.
///
/// In either case a successfully cast object will still implement the mandatory traits.
#[proc_macro]
pub fn check(args: TokenStream) -> TokenStream {
    let cast = parse_macro_input!(args as TraitCastGroup);
    cast.cast_group(CastType::OnlyCheck).into()
}

/// Implement builtin external traits.
#[proc_macro]
pub fn cglue_builtin_ext_traits(_: TokenStream) -> TokenStream {
    cglue_gen::ext::impl_store().into()
}

/// Generate trait implementation for ArcWrappable.
///
/// This is useful for building automatic resource unloading when a trait object gets dropped, for
/// instance a plugin system.
#[proc_macro_attribute]
pub fn cglue_arc_wrappable(_: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);
    gen_wrap(tr, None).into()
}

/// Generate forward trait implementation for Fwd.
///
/// This is useful for using references of trait objects as generic parameters.
#[proc_macro_attribute]
pub fn cglue_forward(_: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);
    gen_forward(tr, None).into()
}

/// Generate trait implementation for ArcWrappable.
///
/// This is useful for building automatic resource unloading when a trait object gets dropped, for
/// instance a plugin system.
#[proc_macro_attribute]
pub fn cglue_arc_wrappable_ext(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as proc_macro2::TokenStream);
    let tr = parse_macro_input!(input as ItemTrait);
    gen_wrap(tr, Some(path)).into()
}

/// Implement #[cglue_arc_wrappable] for all builtin external traits.
#[proc_macro]
pub fn cglue_builtin_ext_wrappable(_: TokenStream) -> TokenStream {
    cglue_gen::ext::impl_ext_wrappable().into()
}

// Marker macros for wrapping

/// Mark the trait or function to use `IntResult`.
///
/// This flag has an effect for functions that return `Result<T, E>`, and
/// is valid when `E` implements `IntResult`. Using this attribute results
/// in more efficient code generation.
#[proc_macro_attribute]
pub fn int_result(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Exclude a single function from using `IntResult`.
#[proc_macro_attribute]
pub fn no_int_result(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Skip reimplementing this function.
#[proc_macro_attribute]
pub fn skip_func(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a custom type.
#[proc_macro_attribute]
pub fn wrap_with(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Specify return type conversion with a closure.
///
/// # Arguments
///
/// A closure that accepts original return value and outputs the defined type.
///
/// If the return type is a reference to the associated type, `ret_tmp` value is available for use
/// to write the intermediate value into.
#[proc_macro_attribute]
pub fn return_wrap(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait object.
#[proc_macro_attribute]
pub fn wrap_with_obj(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait object reference.
#[proc_macro_attribute]
pub fn wrap_with_obj_ref(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait object mutable reference.
#[proc_macro_attribute]
pub fn wrap_with_obj_mut(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait group.
#[proc_macro_attribute]
pub fn wrap_with_group(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait group reference.
///
/// # SAFETY WARNING
///
///
#[proc_macro_attribute]
pub fn wrap_with_group_ref(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with a CGlue trait group mutable reference.
#[proc_macro_attribute]
pub fn wrap_with_group_mut(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Wrap the associated type with ArcWrappable (used by #[cglue_arc_wrappable]).
#[proc_macro_attribute]
pub fn arc_wrap(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}
