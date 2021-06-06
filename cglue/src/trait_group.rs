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
        // Implementors should ensure the same size.
        debug_assert_eq!(
            core::mem::size_of::<Self>(),
            core::mem::size_of::<Self::OpaqueTarget>()
        );

        let input = ManuallyDrop::new(self);

        // We could use a union here, but that forbids us from using Rust 1.45.
        // Rust does optimize this into a no-op anyways
        unsafe { core::ptr::read(&input as *const _ as *const _) }
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

impl<T, V, S> AsRef<V> for CGlueTraitObj<'_, T, V, S> {
    fn as_ref(&self) -> &V {
        &self.vtbl
    }
}

impl<T: Deref<Target = F>, F, V, S> CGlueObjRef<S> for CGlueTraitObj<'_, T, V, S> {
    type ObjType = F;
    type ContType = T;

    fn cobj_ref(&self) -> (&F, &S) {
        (&self.instance, &self.ret_tmp)
    }
}

impl<T: Deref<Target = F> + DerefMut, F, V, S> CGlueObjMut<S> for CGlueTraitObj<'_, T, V, S> {
    fn cobj_mut(&mut self) -> (&mut F, &mut S) {
        (&mut self.instance, &mut self.ret_tmp)
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

pub trait CGlueObjBase {
    type ObjType;
    type ContType: ::core::ops::Deref<Target = Self::ObjType>;
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the constant `this` pointer on the structure.
pub trait CGlueObjRef<S> {
    type ObjType;
    type ContType: ::core::ops::Deref<Target = Self::ObjType>;

    fn cobj_ref(&self) -> (&Self::ObjType, &S);
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the mutable `this` pointer on the structure.
pub trait CGlueObjMut<S>: CGlueObjRef<S> {
    fn cobj_mut(&mut self) -> (&mut Self::ObjType, &mut S);
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the container of the `this` object on the structure.
pub trait CGlueObjOwned<S>: CGlueObjRef<S> {
    fn cobj_owned(self) -> Self::ContType;
}

impl<T: Deref<Target = F> + DerefMut + IntoInner<InnerTarget = F>, F, V, S> CGlueObjOwned<S>
    for CGlueTraitObj<'_, T, V, S>
{
    fn cobj_owned(self) -> T {
        self.instance
    }
}

pub trait CGlueObjBuild<S>: CGlueObjRef<S> {
    /// Construct an object from self vtables and a new object
    ///
    /// # Safety
    ///
    /// It is imporant to make sure `new` uses the same type as the one in the self instance,
    /// because otherwise wrong functions will be invoked.
    unsafe fn cobj_build(&self, new: Self::ContType) -> Self;
}

impl<T: Deref<Target = F>, F, V, S: Default> CGlueObjBuild<S> for CGlueTraitObj<'_, T, V, S> {
    unsafe fn cobj_build(&self, instance: Self::ContType) -> Self {
        Self {
            instance,
            vtbl: self.vtbl,
            ret_tmp: Default::default(),
        }
    }
}

/// Convert a container into inner type.
pub trait IntoInner {
    type InnerTarget;

    /// Consume self and return inner type.
    ///
    /// # Safety
    ///
    /// It might be unsafe to invoke this method if the container has an opaque type, or is on
    /// the wrong side of FFI. CGlue code generator guards against these problems, but it is
    /// important to consider them when working manually with this trait.
    unsafe fn into_inner(self) -> Self::InnerTarget;
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
