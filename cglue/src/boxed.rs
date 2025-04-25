//! # FFI-safe wrapped box.
use crate::slice::CSliceMut;
use crate::trait_group::c_void;
use crate::trait_group::*;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use std::boxed::Box;

/// FFI-safe box
///
/// This box has a static self reference, alongside a custom drop function.
///
/// The drop function can be called from anywhere, it will free on correct allocator internally.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CBox<'a, T: 'a> {
    // TODO: remove these remaps in 0.4
    #[cfg_attr(
        all(feature = "abi_stable10", not(feature = "abi_stable11")),
        sabi(unsafe_change_type = "&'a mut T")
    )]
    #[cfg_attr(feature = "abi_stable11", sabi(unsafe_change_type = &'a mut T))]
    instance: NonNull<T>,
    #[cfg_attr(
        all(feature = "abi_stable10", not(feature = "abi_stable11")),
        sabi(unsafe_change_type = "Option<unsafe extern \"C\" fn(&mut T)>")
    )]
    #[cfg_attr(
        feature = "abi_stable11",
        sabi(unsafe_change_type = Option<unsafe extern "C" fn(&mut T)>)
    )]
    drop_fn: Option<unsafe extern "C" fn(NonNull<T>)>,
    #[cfg_attr(
        all(feature = "abi_stable10", not(feature = "abi_stable11")),
        sabi(unsafe_change_type = "::abi_stable::marker_type::UnsafeIgnoredType<()>")
    )]
    #[cfg_attr(
        feature = "abi_stable11",
        sabi(unsafe_change_type = ::abi_stable::marker_type::UnsafeIgnoredType<()>)
    )]
    _phantom: core::marker::PhantomData<&'a mut T>,
}

unsafe impl<'a, T: 'a> Send for CBox<'a, T> where &'a mut T: Send {}
unsafe impl<'a, T: 'a> Sync for CBox<'a, T> where &'a mut T: Sync {}

impl<T> super::trait_group::IntoInner for CBox<'_, T> {
    type InnerTarget = T;

    unsafe fn into_inner(self) -> Self::InnerTarget {
        let b = Box::from_raw(self.instance.as_ptr());
        std::mem::forget(self);
        *b
    }
}

impl<T> Deref for CBox<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.instance.as_ptr() }
    }
}

impl<T> DerefMut for CBox<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.instance.as_ptr() }
    }
}

impl<T> From<Box<T>> for CBox<'_, T> {
    fn from(this: Box<T>) -> Self {
        let instance = unsafe { NonNull::new_unchecked(Box::into_raw(this)) };
        Self {
            instance,
            drop_fn: Some(cglue_drop_box::<T>),
            _phantom: core::marker::PhantomData,
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

unsafe extern "C" fn cglue_drop_box<T>(this: NonNull<T>) {
    let _ = Box::from_raw(this.as_ptr());
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
