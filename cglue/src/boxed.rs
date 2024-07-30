//! # FFI-safe wrapped box.
use crate::slice::CSliceMut;
use crate::trait_group::c_void;
use crate::trait_group::*;
use core::ops::{Deref, DerefMut};
use std::boxed::Box;

/// FFI-safe box
///
/// This box has a static self reference, alongside a custom drop function.
///
/// The drop function can be called from anywhere, it will free on correct allocator internally.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CBox<'a, T: 'a> {
    instance: &'a mut T,
    drop_fn: Option<unsafe extern "C" fn(&mut T)>,
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
            drop_fn: Some(cglue_drop_box::<T>),
        }
    }
}

impl<T> From<T> for CBox<'_, T> {
    fn from(this: T) -> Self {
        let b = Box::new(this);
        CBox::from(b)
    }
}

// TODO: Remove? Is this even needed?
impl<T> From<(T, NoContext)> for CBox<'_, T> {
    fn from((this, _): (T, NoContext)) -> Self {
        let b = Box::new(this);
        CBox::from(b)
    }
}

impl<T> Drop for CBox<'_, T> {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.drop_fn.take() {
            unsafe { drop_fn(self.instance) };
        }
    }
}

// FIXME: express both Send and !Send box safely (https://github.com/h33p/cglue/issues/18)
unsafe impl<'a, T: Send> Opaquable for CBox<'a, T> {
    type OpaqueTarget = CBox<'a, c_void>;
}

unsafe extern "C" fn cglue_drop_box<T>(this: &mut T) {
    let _ = Box::from_raw(this);
}

/// FFI-safe (unsized) boxed slice
///
/// This box has a static self reference, alongside a custom drop function.
///
/// The drop function can be called from anywhere, it will free on correct allocator internally.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CSliceBox<'a, T: 'a> {
    instance: CSliceMut<'a, T>,
    drop_fn: Option<unsafe extern "C" fn(&mut CSliceMut<'a, T>)>,
}

impl<T> Deref for CSliceBox<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl<T> DerefMut for CSliceBox<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl<T> From<Box<[T]>> for CSliceBox<'_, T> {
    fn from(this: Box<[T]>) -> Self {
        let instance = Box::leak(this).into();
        Self {
            instance,
            drop_fn: Some(cglue_drop_slice_box::<T>),
        }
    }
}

impl<T> Drop for CSliceBox<'_, T> {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.drop_fn.take() {
            unsafe { drop_fn(&mut self.instance) };
        }
    }
}

unsafe impl<'a, T> Opaquable for CSliceBox<'a, T> {
    type OpaqueTarget = CSliceBox<'a, c_void>;
}

unsafe extern "C" fn cglue_drop_slice_box<T>(this: &mut CSliceMut<'_, T>) {
    // SAFETY: we extend the lifetime of the reference but free the underlying data immediately and
    // not use the reference again.
    let extended_instance = (this as *mut CSliceMut<_>).as_mut().unwrap();
    let _ = Box::from_raw(extended_instance.as_slice_mut());
}
