//! Core definitions for traits, and their groups.

use core::ffi::c_void;

/// Simple CGlue trait object.
///
/// This is the simplest form of trait object, represented by a this pointer, and a vtable for
/// single trait.
#[repr(C)]
pub struct CGlueTraitObj<'a, T, V> {
    this: &'a mut T,
    vtbl: &'a V,
}

/// Opaque type of the trait object.
pub type CGlueOpaqueTraitObj<'a, V> = CGlueTraitObj<'a, c_void, <V as CGlueVtbl>::OpaqueVtbl>;

impl<'a, T, V: CGlueVtbl> CGlueTraitObj<'a, T, V> {
    /// Transform self into an opaque version of the trait object.
    ///
    /// The opaque version safely destroys type information, and after this point there is no way
    /// back.
    pub fn into_opaque(self) -> CGlueOpaqueTraitObj<'a, V> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<T, V> AsRef<V> for CGlueTraitObj<'_, T, V> {
    fn as_ref(&self) -> &V {
        &self.vtbl
    }
}

impl<T, V> CGlueObj<T> for CGlueTraitObj<'_, T, V> {
    fn cobj_ref(&self) -> &T {
        &self.this
    }

    fn cobj_mut(&mut self) -> &mut T {
        &mut self.this
    }
}

impl<'a, T: GetCGlueVtbl<'a, V>, V: CGlueVtbl> From<&'a mut T> for CGlueTraitObj<'a, T, V> {
    fn from(this: &'a mut T) -> Self {
        Self {
            this,
            vtbl: T::get_vtbl(),
        }
    }
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the `this` pointer on the structure.
pub trait CGlueObj<T> {
    fn cobj_ref(&self) -> &T;
    fn cobj_mut(&mut self) -> &mut T;
}

/// Trait for CGlue vtables.
///
/// # Safety
///
/// This trait is meant to be implemented by the code generator. If implementing manually, make
/// sure that the `OpaqueVtbl` is the exact same type, with the only difference being `this` types.
pub unsafe trait CGlueVtbl: Sized {
    type OpaqueVtbl: Sized;

    /// Get the opaque vtable for the type.
    fn as_opaque(&self) -> &Self::OpaqueVtbl {
        unsafe { &*(self as *const Self as *const Self::OpaqueVtbl) }
    }
}

/// Build a vtable for the object.
pub trait GetCGlueVtbl<'a, T: CGlueVtbl> {
    /// Builds the wanted vtable.
    fn get_vtbl() -> &'a T;
}
