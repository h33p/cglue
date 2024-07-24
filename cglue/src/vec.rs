use core::mem::ManuallyDrop;
use std::prelude::v1::*;

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CVec<T> {
    data: *mut T,
    len: usize,
    capacity: usize,
    drop_fn: Option<unsafe extern "C" fn(*mut T, usize, usize)>,
    reserve_fn: extern "C" fn(&mut CVec<T>, size: usize) -> usize,
}

unsafe impl<T: Send> Send for CVec<T> {}
unsafe impl<T: Sync> Sync for CVec<T> {}

impl<T> From<Vec<T>> for CVec<T> {
    fn from(mut vec: Vec<T>) -> Self {
        let data = vec.as_mut_ptr();
        let len = vec.len();
        let capacity = vec.capacity();
        core::mem::forget(vec);
        Self {
            data,
            len,
            capacity,
            drop_fn: Some(cglue_drop_vec::<T>),
            reserve_fn: cglue_reserve_vec::<T>,
        }
    }
}

impl<T: Clone> Clone for CVec<T> {
    fn clone(&self) -> Self {
        Self::from(Vec::from(&**self))
    }
}

impl<T> Default for CVec<T> {
    fn default() -> Self {
        Self::from(Vec::new())
    }
}

impl<T> Drop for CVec<T> {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.drop_fn {
            unsafe { drop_fn(self.data, self.len, self.capacity) }
        }
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for CVec<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<T> core::ops::Deref for CVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.data, self.len) }
    }
}

impl<T> core::ops::DerefMut for CVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }
}

impl<T> CVec<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn as_ptr(&self) -> *const T {
        self.data
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.data
    }

    pub fn push(&mut self, value: T) {
        self.reserve(1);
        unsafe { core::ptr::write(self.data.add(self.len), value) };
        self.len += 1;
    }

    /// Insert into the vector
    ///
    /// # Examples
    ///
    /// ```
    /// use cglue::vec::CVec;
    ///
    /// let a: Vec<u32> = vec![1, 2, 3];
    /// let mut cvec = CVec::from(a);
    /// cvec.insert(1, 2);
    ///
    /// assert_eq!(&cvec[..], &[1, 2, 2, 3]);
    /// ```
    pub fn insert(&mut self, index: usize, element: T) {
        assert!(index <= self.len);

        self.reserve(1);
        unsafe {
            let p = self.data.add(index);
            core::ptr::copy(p, p.offset(1), self.len - index);
            core::ptr::write(p, element);
        }
        self.len += 1;
    }

    pub fn reserve(&mut self, additional: usize) {
        if self.capacity - self.len < additional {
            (self.reserve_fn)(self, additional);
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(core::ptr::read(self.data.add(self.len))) }
        }
    }

    /// Insert into the vector
    ///
    /// # Examples
    ///
    /// ```
    /// use cglue::vec::CVec;
    ///
    /// let a: Vec<u32> = vec![1, 2, 3];
    /// let mut cvec = CVec::from(a);
    /// cvec.remove(1);
    ///
    /// assert_eq!(&cvec[..], &[1, 3]);
    /// ```
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len);

        unsafe {
            let ptr = self.data.add(index);
            let ret = core::ptr::read(ptr);
            core::ptr::copy(ptr.offset(1), ptr, self.len - index - 1);
            self.len -= 1;
            ret
        }
    }
}

struct TempVec<'a, T>(ManuallyDrop<Vec<T>>, &'a mut CVec<T>);

impl<'a, T> From<&'a mut CVec<T>> for TempVec<'a, T> {
    fn from(vec: &'a mut CVec<T>) -> Self {
        Self(
            ManuallyDrop::new(unsafe { Vec::from_raw_parts(vec.data, vec.len, vec.capacity) }),
            vec,
        )
    }
}

impl<'a, T> core::ops::Deref for TempVec<'a, T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> core::ops::DerefMut for TempVec<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, T> Drop for TempVec<'a, T> {
    fn drop(&mut self) {
        self.1.data = self.0.as_mut_ptr();
        self.1.len = self.0.len();
        self.1.capacity = self.0.capacity();
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for CVec<T>
where
    T: serde::Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.collect_seq(&**self)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for CVec<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::deserialize(deserializer).map(<_>::into)
    }
}

unsafe extern "C" fn cglue_drop_vec<T>(data: *mut T, len: usize, capacity: usize) {
    let _ = Vec::from_raw_parts(data, len, capacity);
}

extern "C" fn cglue_reserve_vec<T>(vec: &mut CVec<T>, size: usize) -> usize {
    let mut vec = TempVec::from(vec);
    vec.reserve(size);
    vec.capacity()
}
