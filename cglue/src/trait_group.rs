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
pub struct CGlueTraitObj<'a, T, V> {
    instance: T,
    vtbl: &'a V,
}

union Opaquifier<T: Opaquable> {
    input: ManuallyDrop<T>,
    output: ManuallyDrop<T::OpaqueTarget>,
}

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
    CGlueTraitObj<'a, CBox<c_void>, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObjOutRef<'a, V> =
    CGlueTraitObj<'a, &'a c_void, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObjOutMut<'a, V> =
    CGlueTraitObj<'a, &'a mut c_void, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObj<'a, T, V> =
    CGlueTraitObj<'a, <T as Opaquable>::OpaqueTarget, <V as CGlueBaseVtbl>::OpaqueVtbl>;

unsafe impl<'a, T: Opaquable, F: CGlueBaseVtbl> Opaquable for CGlueTraitObj<'a, T, F> {
    type OpaqueTarget = CGlueTraitObj<'a, T::OpaqueTarget, F::OpaqueVtbl>;
}

impl<T, V> AsRef<V> for CGlueTraitObj<'_, T, V> {
    fn as_ref(&self) -> &V {
        &self.vtbl
    }
}

impl<T: Deref<Target = F>, F, V> CGlueObjRef<F> for CGlueTraitObj<'_, T, V> {
    fn cobj_ref(&self) -> &F {
        self.instance.deref()
    }
}

impl<T: Deref<Target = F> + DerefMut, F, V> CGlueObjMut<F> for CGlueTraitObj<'_, T, V> {
    fn cobj_mut(&mut self) -> &mut F {
        self.instance.deref_mut()
    }
}

impl<'a, T: Deref<Target = F>, F: 'a, V: CGlueVtbl<F>> From<T> for CGlueTraitObj<'a, T, V>
where
    &'a V: Default,
{
    fn from(instance: T) -> Self {
        Self {
            instance,
            vtbl: Default::default(),
        }
    }
}

impl<'a, T, V: CGlueVtbl<T>> From<T> for CGlueTraitObj<'a, CBox<T>, V>
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
pub trait CGlueObjRef<T> {
    fn cobj_ref(&self) -> &T;
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the mutable `this` pointer on the structure.
pub trait CGlueObjMut<T>: CGlueObjRef<T> {
    fn cobj_mut(&mut self) -> &mut T;
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

    /// Get the opaque vtable for the type.
    fn as_opaque(&self) -> &Self::OpaqueVtbl {
        unsafe { &*(self as *const Self as *const Self::OpaqueVtbl) }
    }
}
