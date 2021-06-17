//! Core definitions for traits, and their groups.

use crate::boxed::{CBox, CtxBox};
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

/// Opaque type of the trait object.
pub type CGlueOpaqueTraitObjOutCBox<'a, V> = CGlueTraitObj<
    'a,
    CBox<'a, c_void>,
    <V as CGlueBaseVtbl>::OpaqueVtbl,
    <V as CGlueBaseVtbl>::RetTmp,
>;

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

pub trait GetVtbl<V> {
    fn get_vtbl(&self) -> &V;
}

impl<T, V, S> GetVtbl<V> for CGlueTraitObj<'_, T, V, S> {
    fn get_vtbl(&self) -> &V {
        &self.vtbl
    }
}

impl<'a, T: Deref<Target = F> + ContextRef<Context = C>, F, C: Clone, V: CGlueVtbl<F, C>> From<T>
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

impl<'a, T, V: CGlueVtbl<T, NoContext>> From<T> for CGlueTraitObj<'a, CBox<'a, T>, V, V::RetTmp>
where
    &'a V: Default,
{
    fn from(this: T) -> Self {
        Self::from(CBox::from(this))
    }
}

impl<'a, T, V: CGlueVtbl<T, C>, C: Clone> From<(T, C)>
    for CGlueTraitObj<'a, CtxBox<'a, T, C>, V, V::RetTmp>
where
    &'a V: Default,
{
    fn from((this, ctx): (T, C)) -> Self {
        Self::from(CtxBox::from((this, ctx)))
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
    type Context: Clone;

    fn cobj_ref(&self) -> (&Self::ObjType, &S, &Self::Context);
}

impl<'a, T: ContextRef<ObjType = F> + Deref<Target = F>, F, V, S> CGlueObjRef<S>
    for CGlueTraitObj<'_, T, V, S>
{
    type ObjType = F;
    type ContType = T;
    type Context = T::Context;

    fn cobj_ref(&self) -> (&F, &S, &Self::Context) {
        let (obj, ctx) = self.instance.split_ctx_ref();
        (obj, &self.ret_tmp, ctx)
    }
}

pub trait ContextRef {
    type ObjType;
    type Context: Clone;

    fn split_ctx_ref(&self) -> (&Self::ObjType, &Self::Context);
}

impl<'a, T> ContextRef for &'a T {
    type ObjType = T;
    type Context = NoContext;

    fn split_ctx_ref(&self) -> (&Self::ObjType, &Self::Context) {
        (self, &std::marker::PhantomData)
    }
}

impl<'a, T> ContextRef for &'a mut T {
    type ObjType = T;
    type Context = NoContext;

    fn split_ctx_ref(&self) -> (&Self::ObjType, &Self::Context) {
        (self, &std::marker::PhantomData)
    }
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the mutable `this` pointer on the structure.
pub trait CGlueObjMut<S>: CGlueObjRef<S> {
    fn cobj_mut(&mut self) -> (&mut Self::ObjType, &mut S, &Self::Context);
}

impl<'a, T: ContextRef<ObjType = F> + ContextMut + Deref<Target = F> + DerefMut, F, V, S>
    CGlueObjMut<S> for CGlueTraitObj<'_, T, V, S>
{
    fn cobj_mut(&mut self) -> (&mut F, &mut S, &Self::Context) {
        let (obj, ctx) = self.instance.split_ctx_mut();
        (obj, &mut self.ret_tmp, ctx)
    }
}

pub trait ContextMut: ContextRef {
    fn split_ctx_mut(&mut self) -> (&mut Self::ObjType, &Self::Context);
}

impl<'a, T> ContextMut for &'a mut T {
    fn split_ctx_mut(&mut self) -> (&mut Self::ObjType, &Self::Context) {
        (self, &std::marker::PhantomData)
    }
}

/// CGlue compatible object.
///
/// This trait allows to retrieve the container of the `this` object on the structure.
pub trait CGlueObjOwned<S>: CGlueObjMut<S> {
    fn cobj_owned(self) -> Self::ContType;
}

impl<
        'a,
        T: ContextRef<ObjType = F> + ContextMut + ContextOwned + Deref<Target = F> + DerefMut,
        F,
        V,
        S,
    > CGlueObjOwned<S> for CGlueTraitObj<'_, T, V, S>
{
    fn cobj_owned(self) -> T {
        self.instance
    }
}

pub trait ContextOwned: ContextMut {
    /// Split the container into its underlying object and the context.
    ///
    /// # Safety
    ///
    /// It is crucial to invoke this method where type information is known and
    /// was not destroyed.
    unsafe fn split_ctx_owned(self) -> (Self::ObjType, Self::Context);
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

impl<T: Deref<Target = F> + ContextRef<ObjType = F>, F, V, S: Default> CGlueObjBuild<S>
    for CGlueTraitObj<'_, T, V, S>
{
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
pub trait CGlueVtbl<T, C>: CGlueBaseVtbl {}

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

/// Describes absence of a context.
///
/// This context is used for regular `CBox` trait objects as well as by-ref or by-mut objects.
pub type NoContext = std::marker::PhantomData<c_void>;

unsafe impl Opaquable for NoContext {
    type OpaqueTarget = NoContext;
}

unsafe impl Opaquable for () {
    type OpaqueTarget = ();
}

unsafe impl Opaquable for c_void {
    type OpaqueTarget = c_void;
}
