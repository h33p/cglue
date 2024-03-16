//! CGlue procedural macros

extern crate proc_macro;

use cglue_gen::ext::{ext_abs_remap, prelude_remap_with_ident};
use cglue_gen::forward::gen_forward;
use cglue_gen::generics::{GenericCastType, GroupCastType};
use cglue_gen::trait_groups::*;
use proc_macro::TokenStream;
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

    let ext_ident = format_ident!("{}Ext", tr.ident);

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
        mut target,
        expr,
        ident,
    } = parse_macro_input!(args as GenericCastType);

    if let Ok(ident) = parse2::<Ident>(ident) {
        target.path = ext_abs_remap(prelude_remap_with_ident(target.path, &ident))
    }

    let gen = quote! {
        #crate_path::trait_group::Opaquable::into_opaque({
            // We need rust to infer lifetimes and generics, thus we use a wrapper trait
            use #crate_path::from2::From2;
            #target ::from2(#expr)
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
/// 3.1. If the same trait is listed twice (with different generic parameters), it may be aliased
///   with `OrigTrait<Generic> = TraitAlias`. Then, all subsequent operations, such as `cast!` need
///   to use the alias, as opposed to the original trait.
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
///    traits. Note that the list must redefine identical aliases, as defined in
///    `cglue_trait_group!` invokation.
#[proc_macro]
#[cfg_attr(feature = "unstable", allow(unused))]
pub fn cglue_impl_group(args: TokenStream) -> TokenStream {
    #[cfg(not(feature = "unstable"))]
    {
        let args = parse_macro_input!(args as TraitGroupImpl);
        args.implement_group().into()
    }
    #[cfg(feature = "unstable")]
    TokenStream::new()
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

    let GroupCastType {
        mut target,
        expr,
        ident,
    } = parse_macro_input!(args as GroupCastType);

    if let Ok(ident) = parse2::<Ident>(ident) {
        target.path = ext_abs_remap(prelude_remap_with_ident(target.path, &ident));
    }

    let gen = quote! {
        #crate_path::trait_group::Opaquable::into_opaque({
            // We need rust to infer lifetimes and generics, thus we use a wrapper trait
            use #crate_path::from2::From2;
            #target ::from2(#expr)
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

/// Generate forward trait implementation for Fwd.
///
/// This is useful for using references of trait objects as generic parameters.
#[proc_macro_attribute]
pub fn cglue_forward(_: TokenStream, input: TokenStream) -> TokenStream {
    let tr = parse_macro_input!(input as ItemTrait);
    gen_forward(tr, None).into()
}

/// Generate forward trait implementation for Fwd.
///
/// This is useful for using references of trait objects as generic parameters.
#[proc_macro_attribute]
pub fn cglue_forward_ext(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as proc_macro2::TokenStream);
    let tr = parse_macro_input!(input as ItemTrait);
    gen_forward(tr, Some(path)).into()
}

/// Implement [macro@cglue_forward_ext] for all builtin external traits.
#[proc_macro]
pub fn cglue_builtin_ext_forward(_: TokenStream) -> TokenStream {
    cglue_gen::ext::impl_ext_forward().into()
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

/// Add custom wrapping for a trait impl and the C interface.
///
/// This is a pretty complex, and fine-grained operation. It allows to control almost every aspect
/// of type wrapping. The logic of argument layout is a declaration and sequence of actions
/// from top to bottom.
///
/// In addition, this is the only way to allow using generic parameters within functions - C
/// implementation must have them converted to concrete types beforehand.
///
/// Example from [cglue-gen](cglue_gen::ext::core::fmt).
///
/// ```ignore
/// #[custom_impl(
///     // Types within the C interface other than self and additional wrappers.
///     {
///         f_out: &mut WriteMut,
///     },
///     // Unwrapped return type
///     Result<(), ::core::fmt::Error>,
///     // Conversion in trait impl to C arguments (signature names are expected).
///     {
///         let f_out: WriteBaseMut<::core::fmt::Formatter> = From::from(f);
///         let f_out = &mut #crate_path::trait_group::Opaquable::into_opaque(f_out);
///     },
///     // This is the body of C impl minus the automatic wrapping.
///     {
///         write!(f_out, #fmt_str, this)
///     },
///     // This part is processed in the trait impl after the call returns (impl_func_ret,
///     // nothing extra needs to happen here).
///     {
///     },
/// )]
/// ```
#[proc_macro_attribute]
pub fn custom_impl(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// Emit a vtable entry, but do not use it in Rust.
///
/// This allows to expose functionality to C/C++ users with slight changes in return types,
/// while making use of the blanket implementation in Rust. It is necessary when a function
/// itself wraps `Self` into some specific type, and would otherwise be completely
/// incompatible with `CGlue`.
///
/// User is able to specify one of the wrapping macros to configure the behavior:
///
/// ```ignore
/// #[vtbl_only(wrap_with_obj(ExampleTrait))]
/// ```
///
/// Note that there could be some parity issues between Rust and C/C++ APIs, because in Rust the
/// blanket implementation will be invoked, while in FFI the underlying implementation will be
/// called.
#[proc_macro_attribute]
pub fn vtbl_only(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}
