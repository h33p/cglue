//! Describes an FFI-safe wrapped box.
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

unsafe impl<'a, T> Opaquable for CBox<'a, T> {
    type OpaqueTarget = CBox<'a, c_void>;
}

unsafe extern "C" fn cglue_drop_box<T>(this: &mut T) {
    let _ = Box::from_raw(this);
}
