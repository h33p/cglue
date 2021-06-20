//! Describes slices as C-structs.

use core::marker::PhantomData;

/// Wrapper around const slices.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CSliceRef<'a, T> {
    data: *const T,
    len: usize,
    _lifetime: PhantomData<&'a T>,
}

unsafe impl<'a, T> Send for CSliceRef<'a, T> where T: Send {}
unsafe impl<'a, T> Sync for CSliceRef<'a, T> where T: Sync {}

impl<'a, T> CSliceRef<'a, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub const fn as_ptr(&self) -> *const T {
        self.data
    }

    pub const fn as_mut_ptr(&self) -> *mut T {
        self.data as *mut T
    }

    pub fn as_slice(&'a self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.data, self.len) }
    }
}

impl<'a, T> From<&'a [T]> for CSliceRef<'a, T> {
    fn from(from: &'a [T]) -> Self {
        Self {
            data: from.as_ptr(),
            len: from.len(),
            _lifetime: PhantomData::default(),
        }
    }
}

impl<'a, T> From<CSliceRef<'a, T>> for &'a [T] {
    fn from(from: CSliceRef<'a, T>) -> Self {
        unsafe { core::slice::from_raw_parts(from.data, from.len) }
    }
}

impl<T> std::ops::Deref for CSliceRef<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.data, self.len) }.into()
    }
}

/// Wrapper around mutable slices.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CSliceMut<'a, T> {
    data: *mut T,
    len: usize,
    _lifetime: PhantomData<&'a T>,
}

unsafe impl<'a, T> Send for CSliceMut<'a, T> where T: Send {}
unsafe impl<'a, T> Sync for CSliceMut<'a, T> where T: Sync {}

impl<'a, T> CSliceMut<'a, T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub const fn as_ptr(&self) -> *const T {
        self.data as *const T
    }

    pub const fn as_mut_ptr(&self) -> *mut T {
        self.data
    }

    pub fn as_slice(&'a self) -> &'a mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }
}

impl<'a, T> From<&'a mut [T]> for CSliceMut<'a, T> {
    fn from(from: &'a mut [T]) -> Self {
        Self {
            data: from.as_mut_ptr(),
            len: from.len(),
            _lifetime: PhantomData::default(),
        }
    }
}

impl<'a, T> From<CSliceMut<'a, T>> for &'a [T] {
    fn from(from: CSliceMut<'a, T>) -> Self {
        unsafe { core::slice::from_raw_parts(from.data, from.len) }
    }
}

impl<'a, T> From<CSliceMut<'a, T>> for &'a mut [T] {
    fn from(from: CSliceMut<'a, T>) -> Self {
        unsafe { core::slice::from_raw_parts_mut(from.data, from.len) }
    }
}

impl<'a, T> std::ops::Deref for CSliceMut<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.data, self.len) }.into()
    }
}

impl<'a, T> std::ops::DerefMut for CSliceMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }.into()
    }
}
