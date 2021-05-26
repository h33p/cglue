//! Core definitions for traits, and their groups.

use crate::wrap_box::CBox;
use core::ffi::c_void;
use core::ops::{Deref, DerefMut};

/// Simple CGlue trait object.
///
/// This is the simplest form of trait object, represented by a this pointer, and a vtable for
/// single trait.
///
/// `this` value usually is either a reference, or a mutable reference, or a `CBox`, which
/// contains static reference to the instance, and a dedicated drop function for freeing resources.
#[repr(C)]
pub struct CGlueTraitObj<'a, T, V> {
    this: T,
    vtbl: &'a V,
}

/// Opaque type of the trait object.
pub type CGlueOpaqueTraitObjOutCBox<'a, V> =
    CGlueTraitObj<'a, CBox<c_void>, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObjOutRef<'a, V> =
    CGlueTraitObj<'a, &'a c_void, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObjOutMut<'a, V> =
    CGlueTraitObj<'a, &'a mut c_void, <V as CGlueBaseVtbl>::OpaqueVtbl>;

pub type CGlueOpaqueTraitObj<'a, V> = CGlueTraitObj<'a, c_void, V>;

impl<'a, T, V: CGlueVtbl<T>> CGlueTraitObj<'a, &'a mut T, V> {
    /// Transform self into an opaque version of the trait object.
    ///
    /// The opaque version safely destroys type information, and after this point there is no way
    /// back.
    pub fn into_opaque(self) -> CGlueOpaqueTraitObjOutMut<'a, V> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<'a, T, V: CGlueVtbl<T>> CGlueTraitObj<'a, &'a T, V> {
    /// Transform self into an opaque version of the trait object.
    ///
    /// The opaque version safely destroys type information, and after this point there is no way
    /// back.
    pub fn into_opaque(self) -> CGlueOpaqueTraitObjOutRef<'a, V> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<'a, T, V: CGlueVtbl<T>> CGlueTraitObj<'a, CBox<T>, V> {
    /// Transform self into an opaque version of the trait object.
    ///
    /// The opaque version safely destroys type information, and after this point there is no way
    /// back.
    pub fn into_opaque(self) -> CGlueOpaqueTraitObjOutCBox<'a, V> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<T, V> AsRef<V> for CGlueTraitObj<'_, T, V> {
    fn as_ref(&self) -> &V {
        &self.vtbl
    }
}

impl<T: Deref<Target = F>, F, V> CGlueObjRef<F> for CGlueTraitObj<'_, T, V> {
    fn cobj_ref(&self) -> &F {
        self.this.deref()
    }
}

impl<T: Deref<Target = F> + DerefMut, F, V> CGlueObjMut<F> for CGlueTraitObj<'_, T, V> {
    fn cobj_mut(&mut self) -> &mut F {
        self.this.deref_mut()
    }
}

impl<'a, T, V: CGlueVtbl<T>> From<&'a mut T> for CGlueTraitObj<'a, &'a mut T, V>
where
    &'a V: Default,
{
    fn from(this: &'a mut T) -> Self {
        Self {
            this,
            vtbl: Default::default(),
        }
    }
}

impl<'a, T, V: CGlueVtbl<T>> From<&'a T> for CGlueTraitObj<'a, &'a T, V>
where
    &'a V: Default,
{
    fn from(this: &'a T) -> Self {
        Self {
            this,
            vtbl: Default::default(),
        }
    }
}

impl<'a, T, V: CGlueVtbl<T>> From<T> for CGlueTraitObj<'a, CBox<T>, V>
where
    &'a V: Default,
{
    fn from(this: T) -> Self {
        Self {
            this: CBox::from(this),
            vtbl: Default::default(),
        }
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
