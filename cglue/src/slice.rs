//! Describes slices as C-structs.
//!
//! These slices are then transferable across the FFI boundary safely.

use core::marker::PhantomData;

/// Wrapper around const slices.
///
/// This is meant as a safe type to pass across the FFI boundary with similar semantics as regular
/// slice. However, not all functionality is present, use the slice conversion functions.
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
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
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

impl<'a> CSliceRef<'a, u8> {
    pub fn as_str(self) -> &'a str {
        self.into()
    }
}

impl<'a> From<&'a str> for CSliceRef<'a, u8> {
    fn from(from: &'a str) -> Self {
        Self {
            data: from.as_ptr(),
            len: from.len(),
            _lifetime: PhantomData::default(),
        }
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

impl<'a> From<CSliceRef<'a, u8>> for &'a str {
    fn from(from: CSliceRef<'a, u8>) -> Self {
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(from.data, from.len)) }
    }
}

impl<T> std::ops::Deref for CSliceRef<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Wrapper around mutable slices.
///
/// This is meant as a safe type to pass across the FFI boundary with similar semantics as regular
/// slice. However, not all functionality is present, use the slice conversion functions.
#[repr(C)]
pub struct CSliceMut<'a, T> {
    data: *mut T,
    len: usize,
    _lifetime: PhantomData<&'a T>,
}

unsafe impl<'a, T> Send for CSliceMut<'a, T> where T: Send {}
unsafe impl<'a, T> Sync for CSliceMut<'a, T> where T: Sync {}

impl<'a, T> CSliceMut<'a, T> {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn as_ptr(&self) -> *const T {
        self.data as *const T
    }

    pub const fn as_mut_ptr(&self) -> *mut T {
        self.data
    }

    pub fn as_slice(&'a self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.data, self.len) }
    }

    pub fn as_slice_mut(&'a mut self) -> &'a mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }
}

impl<'a> CSliceMut<'a, u8> {
    pub fn as_str(self) -> &'a str {
        self.into()
    }

    pub fn as_mut_str(self) -> &'a mut str {
        self.into()
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

impl<'a> From<&'a mut str> for CSliceMut<'a, u8> {
    fn from(from: &'a mut str) -> Self {
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

impl<'a> From<CSliceMut<'a, u8>> for &'a str {
    fn from(from: CSliceMut<'a, u8>) -> Self {
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(from.data, from.len)) }
    }
}

impl<'a, T> From<CSliceMut<'a, T>> for &'a mut [T] {
    fn from(from: CSliceMut<'a, T>) -> Self {
        unsafe { core::slice::from_raw_parts_mut(from.data, from.len) }
    }
}

impl<'a> From<CSliceMut<'a, u8>> for &'a mut str {
    fn from(from: CSliceMut<'a, u8>) -> Self {
        unsafe {
            core::str::from_utf8_unchecked_mut(core::slice::from_raw_parts_mut(from.data, from.len))
        }
    }
}

impl<'a, T> std::ops::Deref for CSliceMut<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T> std::ops::DerefMut for CSliceMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }
}
