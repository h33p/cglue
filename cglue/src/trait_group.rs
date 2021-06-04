//! Core definitions for traits, and their groups.

use crate::boxed::CBox;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

/// Simple CGlue trait object.
///
/// This is the simplest form of trait object, represented by a this pointer, and a vtable for
/// single trait.
///
/// `instance` value usually is either a reference, or a mutable reference, or a `CBox`, which
/// contains static reference to the instance, and a dedicated drop function for freeing resources.
#[repr(C)]
pub struct CGlueTraitObj<'a, T, V, S> {
    instance: T,
    vtbl: &'a V,
    ret_tmp: S,
}

union Opaquifier<T: Opaquable> {
    input: ManuallyDrop<T>,
    output: ManuallyDrop<T::OpaqueTarget>,
}

/// Describes an opaquable object.
///
/// This trait provides a safe many-traits-to-one conversion. For instance, concrete vtable types
/// get converted to `c_void` types, and so on.
///
/// # Safety
///
/// Implementor of this trait must ensure the same layout of regular and opaque data. Generally,
/// this means using the same structure, but taking type T and converting it to c_void, but it is
/// not limited to that.
pub unsafe trait Opaquable: Sized {
    type OpaqueTarget;

    /// Transform self into an opaque version of the trait object.
    ///
    /// The opaque version safely destroys type information, and after this point there is no way
    /// back.
    fn into_opaque(self) -> Self::OpaqueTarget {
        let val = Opaquifier {
            input: ManuallyDrop::new(self),
        };

        // Implementors should ensure the same size.
        debug_assert_eq!(
            core::mem::size_of::<Self>(),
            core::mem::size_of::<Self::OpaqueTarget>()
        );

        unsafe { ManuallyDrop::into_inner(val.output) }
    }
}

unsafe impl<'a, T> Opaquable for &'a T {
    type OpaqueTarget = &'a c_void;
}

unsafe impl<'a, T> Opaquable for &'a mut T {
    type OpaqueTarget = &'a mut c_void;
}

unsafe impl<'a, T> Opaquable for CBox<T> {
    type OpaqueTarget = CBox<c_void>;
}

/// Opaque type of the trait object.
pub type CGlueOpaqueTraitObjOutCBox<'a, V> =
    CGlueTraitObj<'a, CBox<c_void>, <V as CGlueBaseVtbl>::OpaqueVtbl, <V as CGlueBaseVtbl>::RetTmp>;

pub type CGlueOpaqueTraitObjOutRef<'a, V> =
    CGlueTraitObj<'a, &'a c_void, <V as CGlueBaseVtbl>::OpaqueVtbl, <V as CGlueBaseVtbl>::RetTmp>;

pub type CGlueOpaqueTraitObjOutMut<'a, V> = CGlueTraitObj<
    'a,
    &'a mut c_void,
    <V as CGlueBaseVtbl>::OpaqueVtbl,
    <V as CGlueBaseVtbl>::RetTmp,
>;

pub type CGlueOpaqueTraitObj<'a, T, V> = CGlueTraitObj<
    'a,
    <T as Opaquable>::OpaqueTarget,
    <V as CGlueBaseVtbl>::OpaqueVtbl,
    <V as CGlueBaseVtbl>::RetTmp,
>;

unsafe impl<'a, T: Opaquable, F: CGlueBaseVtbl> Opaquable for CGlueTraitObj<'a, T, F, F::RetTmp> {
    type OpaqueTarget = CGlueTraitObj<'a, T::OpaqueTarget, F::OpaqueVtbl, F::RetTmp>;
}

pub trait CGlueObj<V, S> {
    fn vtbl_ref(&self) -> &V;
    fn ret_tmp_ref(&self) -> &S;
    fn ret_tmp_mut(&mut self) -> &mut S;
}

impl<T, V, S> CGlueObj<V, S> for CGlueTraitObj<'_, T, V, S> {
    fn vtbl_ref(&self) -> &V {
        &self.vtbl
    }

    fn ret_tmp_ref(&self) -> &S {
        &self.ret_tmp
    }

    fn ret_tmp_mut(&mut self) -> &mut S {
        &mut self.ret_tmp
    }
}

impl<T: Deref<Target = F>, F, V, S> CGlueObjRef<F, S> for CGlueTraitObj<'_, T, V, S> {
    fn cobj_ref(&self) -> (&F, &S) {
        (self.instance.deref(), &self.ret_tmp)
    }
}

impl<T: Deref<Target = F> + DerefMut, F, V, S> CGlueObjMut<F, S> for CGlueTraitObj<'_, T, V, S> {
    fn cobj_mut(&mut self) -> (&mut F, &mut S) {
        (self.instance.deref_mut(), &mut self.ret_tmp)
    }
}

impl<'a, T: Deref<Target = F>, F: 'a, V: CGlueVtbl<F>> From<T>
    for CGlueTraitObj<'a, T, V, V::RetTmp>
where
    &'a V: Default,
{
    fn from(instance: T) -> Self {
        Self {
            instance,
            vtbl: Default::default(),
            ret_tmp: Default::default(),
        }
    }
}

impl<'a, T, V: CGlueVtbl<T>> From<T> for CGlueTraitObj<'a, CBox<T>, V, V::RetTmp>
where
    &'a V: Default,
{
    fn from(this: T) -> Self {
        Self::from(CBox::from(this))
    }
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the constant `this` pointer on the structure.
pub trait CGlueObjRef<T, S> {
    fn cobj_ref(&self) -> (&T, &S);
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the mutable `this` pointer on the structure.
pub trait CGlueObjMut<T, S>: CGlueObjRef<T, S> {
    fn cobj_mut(&mut self) -> (&mut T, &mut S);
}

/// Trait for CGlue vtables.
pub trait CGlueVtbl<T>: CGlueBaseVtbl {}

/// Trait for CGlue vtables.
///
/// # Safety
///
/// This trait is meant to be implemented by the code generator. If implementing manually, make
/// sure that the `OpaqueVtbl` is the exact same type, with the only difference being `this` types.
pub unsafe trait CGlueBaseVtbl: Sized {
    type OpaqueVtbl: Sized;
    type RetTmp: Sized + Default;

    /// Get the opaque vtable for the type.
    fn as_opaque(&self) -> &Self::OpaqueVtbl {
        unsafe { &*(self as *const Self as *const Self::OpaqueVtbl) }
    }
}
