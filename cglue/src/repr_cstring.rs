//! # Null-terminated transparent C-strings.

use std::prelude::v1::*;
use std::ptr::NonNull;
use std::slice::*;
use std::str::from_utf8_unchecked;

#[cfg(feature = "std")]
use std::os::raw::c_char;
#[cfg(not(feature = "std"))]
#[allow(non_camel_case_types)]
pub type c_char = i8;

/// Wrapper around null-terminated C-style strings.
///
/// Analog to Rust's `String`, [`ReprCString`] owns the underlying data.
#[repr(transparent)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct ReprCString(NonNull<c_char>);

// The underlying pointer isn't being mutated after construction,
// hence it is safe to assume access to the raw pointer is both Send + Sync
unsafe impl Send for ReprCString {}
unsafe impl Sync for ReprCString {}

unsafe fn string_size(mut ptr: *const c_char) -> usize {
    (1..)
        .take_while(|_| {
            let ret = *ptr;
            ptr = ptr.offset(1);
            ret != 0
        })
        .last()
        .unwrap_or(0)
        + 1
}

impl From<&[u8]> for ReprCString {
    fn from(from: &[u8]) -> Self {
        let b = Box::new(from.to_vec().into_boxed_slice());
        Self(NonNull::new(Box::leak(b).as_mut_ptr() as *mut _).unwrap())
    }
}

impl From<&str> for ReprCString {
    fn from(from: &str) -> Self {
        let b = from
            .bytes()
            .take_while(|&b| b != 0)
            .chain(Some(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self(NonNull::new(Box::leak(b).as_mut_ptr() as *mut _).unwrap())
    }
}

impl From<String> for ReprCString {
    fn from(from: String) -> Self {
        from.as_str().into()
    }
}

impl<'a> std::borrow::Borrow<ReprCStr<'a>> for ReprCString {
    fn borrow(&self) -> &ReprCStr<'a> {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl AsRef<str> for ReprCString {
    fn as_ref(&self) -> &str {
        unsafe {
            from_utf8_unchecked(from_raw_parts(
                self.0.as_ptr() as *const _,
                string_size(self.0.as_ptr()) - 1,
            ))
        }
    }
}

impl std::ops::Deref for ReprCString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl Drop for ReprCString {
    fn drop(&mut self) {
        let _ = unsafe {
            Box::from_raw(from_raw_parts_mut(
                self.0.as_ptr() as *mut _,
                string_size(self.0.as_ptr()),
            ))
        };
    }
}

impl Clone for ReprCString {
    fn clone(&self) -> Self {
        self.as_ref().into()
    }
}

impl std::fmt::Display for ReprCString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.as_ref())
    }
}

impl std::fmt::Debug for ReprCString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReprCString")
            .field("0", &self.as_ref())
            .finish()
    }
}

impl std::hash::Hash for ReprCString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl std::cmp::PartialEq for ReprCString {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl std::cmp::Eq for ReprCString {}

#[cfg(feature = "serde")]
impl serde::Serialize for ReprCString {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReprCString {
    fn deserialize<D>(deserializer: D) -> std::result::Result<ReprCString, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReprCStringVisitor;

        impl<'de> ::serde::de::Visitor<'de> for ReprCStringVisitor {
            type Value = ReprCString;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                Ok(v.into())
            }
        }

        deserializer.deserialize_str(ReprCStringVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::ReprCString;

    #[test]
    fn string_size_matches() {
        assert_eq!(0, ReprCString::from("").as_ref().len());
        assert_eq!(1, ReprCString::from("1").as_ref().len());
        assert_eq!(5, ReprCString::from("12345").as_ref().len());
    }
}

/// Wrapper around null-terminated C-style strings.
///
/// Analog to Rust's `str`, [`ReprCStr`] borrows the underlying data.
#[repr(transparent)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct ReprCStr<'a>(&'a c_char);

#[cfg(feature = "std")]
use std::ffi::CStr;

#[cfg(feature = "std")]
impl<'a> From<&'a CStr> for ReprCStr<'a> {
    fn from(from: &'a CStr) -> Self {
        Self(unsafe { (from.as_ptr() as *const c_char).as_ref() }.unwrap())
    }
}

impl<'a> AsRef<str> for ReprCStr<'a> {
    fn as_ref(&self) -> &str {
        unsafe {
            from_utf8_unchecked(from_raw_parts(
                self.0 as *const _ as *const _,
                string_size(self.0) - 1,
            ))
        }
    }
}

impl<'a> std::fmt::Display for ReprCStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.as_ref())
    }
}

impl<'a> std::fmt::Debug for ReprCStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReprCStr<'a>")
            .field("0", &self.as_ref())
            .finish()
    }
}

impl<'a> std::hash::Hash for ReprCStr<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl<'a> std::cmp::PartialEq for ReprCStr<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl<'a> std::cmp::Eq for ReprCStr<'a> {}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for ReprCStr<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
