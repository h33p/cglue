//! Describes an FFI-safe wrapped box.
use crate::trait_group::*;
use core::ops::{Deref, DerefMut};
use std::ffi::c_void;

/// FFI-safe box
///
/// This box has a static self reference, alongside a custom drop function.
///
/// The drop function can be called from anywhere, it will free on correct allocator internally.
#[repr(C)]
pub struct CBox<'a, T> {
    instance: &'a mut T,
    drop: unsafe extern "C" fn(&mut T),
}

impl<T> super::trait_group::IntoInner for CBox<'_, T> {
    type InnerTarget = T;

    unsafe fn into_inner(self) -> Self::InnerTarget {
        let b = Box::from_raw(self.instance);
        std::mem::forget(self);
        *b
    }
}

impl<T> Deref for CBox<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.instance
    }
}

impl<T> DerefMut for CBox<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.instance
    }
}

impl<T> From<Box<T>> for CBox<'_, T> {
    fn from(this: Box<T>) -> Self {
        let instance = Box::leak(this);
        Self {
            instance,
            drop: cglue_drop_box::<T>,
        }
    }
}

impl<T> From<T> for CBox<'_, T> {
    fn from(this: T) -> Self {
        let b = Box::new(this);
        CBox::from(b)
    }
}

impl<T> From<(T, NoContext)> for CBox<'_, T> {
    fn from((this, _): (T, NoContext)) -> Self {
        let b = Box::new(this);
        CBox::from(b)
    }
}

impl<T> Drop for CBox<'_, T> {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.instance) };
    }
}

unsafe impl<'a, T> Opaquable for CBox<'a, T> {
    type OpaqueTarget = CBox<'a, c_void>;
}

impl<'a, T> ContextRef for CBox<'a, T> {
    type Context = NoContext;
    type ObjType = T;

    fn split_ctx_ref(&self) -> (&Self::ObjType, &Self::Context) {
        (self.instance, &std::marker::PhantomData)
    }
}

impl<'a, T> ContextMut for CBox<'a, T> {
    fn split_ctx_mut(&mut self) -> (&mut Self::ObjType, &Self::Context) {
        (self.instance, &std::marker::PhantomData)
    }
}

impl<'a, T> ContextOwned for CBox<'a, T> {
    unsafe fn split_ctx_owned(self) -> (Self::ObjType, Self::Context) {
        (self.into_inner(), Default::default())
    }
}

unsafe extern "C" fn cglue_drop_box<T>(this: &mut T) {
    let _ = Box::from_raw(this);
}

#[repr(C)]
pub struct CtxBox<'a, T, C> {
    inner: CBox<'a, T>,
    ctx: C,
}

impl<T, C> super::trait_group::IntoInner for CtxBox<'_, T, C> {
    type InnerTarget = T;

    unsafe fn into_inner(mut self) -> Self::InnerTarget {
        let b = Box::from_raw(self.inner.instance);
        std::ptr::drop_in_place(&mut self.ctx);
        std::mem::forget(self);
        *b
    }
}

impl<T, C> Deref for CtxBox<'_, T, C> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.instance
    }
}

impl<T, C> DerefMut for CtxBox<'_, T, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.instance
    }
}

unsafe impl<'a, T, C: Opaquable> Opaquable for CtxBox<'a, T, C> {
    type OpaqueTarget = CtxBox<'a, c_void, C::OpaqueTarget>;
}

impl<'a, T, C: Clone> ContextRef for CtxBox<'a, T, C> {
    type Context = C;
    type ObjType = T;

    fn split_ctx_ref(&self) -> (&Self::ObjType, &Self::Context) {
        (&self.inner, &self.ctx)
    }
}

impl<'a, T, C: Clone> ContextMut for CtxBox<'a, T, C> {
    fn split_ctx_mut(&mut self) -> (&mut Self::ObjType, &Self::Context) {
        (&mut self.inner, &self.ctx)
    }
}

impl<'a, T, C: Clone> ContextOwned for CtxBox<'a, T, C> {
    unsafe fn split_ctx_owned(self) -> (Self::ObjType, Self::Context) {
        let b = Box::from_raw(self.inner.instance);
        let c = self.ctx;
        std::mem::forget(self.inner);
        (*b, c)
    }
}

impl<'a, T, C: Default> From<CBox<'a, T>> for CtxBox<'a, T, C> {
    fn from(inner: CBox<'a, T>) -> Self {
        Self {
            inner,
            ctx: Default::default(),
        }
    }
}

impl<'a, T, C> From<(T, C)> for CtxBox<'a, T, C> {
    fn from((inner, ctx): (T, C)) -> Self {
        Self {
            inner: CBox::from(inner),
            ctx,
        }
    }
}
