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
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CSliceRef<'a, T: 'a> {
    data: *const T,
    len: usize,
    _lifetime: PhantomData<&'a T>,
}

unsafe impl<'a, T> Send for CSliceRef<'a, T> where T: Send {}
unsafe impl<'a, T> Sync for CSliceRef<'a, T> where T: Sync {}

#[cfg(feature = "std")]
impl<T: std::fmt::Debug> std::fmt::Debug for CSliceRef<'_, T>
where
    for<'a> &'a [T]: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("CSliceRef")
            .field("data", &self.data)
            .field("len", &self.len)
            .field("slice", &self.as_slice());
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::fmt::Display for CSliceRef<'_, u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.into_str())
    }
}

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

    pub const fn from_slice(s: &'a [T]) -> Self {
        Self {
            data: s.as_ptr(),
            len: s.len(),
            _lifetime: PhantomData {},
        }
    }
}

impl<'a> CSliceRef<'a, u8> {
    pub const fn from_str(s: &'a str) -> Self {
        Self::from_slice(s.as_bytes())
    }
}

impl<'a> CSliceRef<'a, u8> {
    #[deprecated(since = "0.1.2", note = "use Into::into, or into_str instead")]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_str(self) -> &'a str {
        self.into()
    }

    pub fn into_str(self) -> &'a str {
        self.into()
    }
}

impl<'a> From<&'a str> for CSliceRef<'a, u8> {
    fn from(from: &'a str) -> Self {
        Self::from_str(from)
    }
}

impl<'a, T> From<&'a [T]> for CSliceRef<'a, T> {
    fn from(from: &'a [T]) -> Self {
        Self::from_slice(from)
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
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CSliceMut<'a, T: 'a> {
    data: *mut T,
    len: usize,
    _lifetime: PhantomData<&'a T>,
}

unsafe impl<'a, T> Send for CSliceMut<'a, T> where T: Send {}
unsafe impl<'a, T> Sync for CSliceMut<'a, T> where T: Sync {}

#[cfg(feature = "std")]
impl<T: std::fmt::Debug> std::fmt::Debug for CSliceMut<'_, T>
where
    for<'a> &'a [T]: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.debug_struct("CSliceMut")
            .field("data", &self.data)
            .field("len", &self.len)
            .field("slice", &self.as_slice());
        Ok(())
    }
}

impl std::fmt::Display for CSliceMut<'_, u8> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", unsafe { core::str::from_utf8_unchecked(&*self) })
    }
}

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
    #[deprecated(since = "0.1.2", note = "use Into::into, or into_str instead")]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_str(self) -> &'a str {
        self.into()
    }

    pub fn into_str(self) -> &'a str {
        self.into()
    }

    #[deprecated(since = "0.1.2", note = "use Into::into, or into_mut_str instead")]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_mut_str(self) -> &'a mut str {
        self.into()
    }

    pub fn into_mut_str(self) -> &'a mut str {
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

impl<'a: 'b, 'b, T> From<&'b CSliceMut<'a, T>> for CSliceMut<'a, T> {
    fn from(from: &'b CSliceMut<'a, T>) -> Self {
        Self {
            data: from.data,
            len: from.len,
            _lifetime: from._lifetime,
        }
    }
}

impl<'a: 'b, 'b, T> From<&'b mut CSliceMut<'a, T>> for CSliceMut<'a, T> {
    fn from(from: &'b mut CSliceMut<'a, T>) -> Self {
        Self {
            data: from.data,
            len: from.len,
            _lifetime: from._lifetime,
        }
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
