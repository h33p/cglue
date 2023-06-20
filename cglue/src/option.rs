//! # FFI safe option.

/// FFI-safe Option.
///
/// This type is not really meant for general use, but rather as a last-resort conversion for type
/// wrapping.
///
/// Typical workflow would include temporarily converting into/from COption.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
pub enum COption<T> {
    None,
    Some(T),
}

impl<T> Default for COption<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<Option<T>> for COption<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            None => Self::None,
            Some(t) => Self::Some(t),
        }
    }
}

impl<T> From<COption<T>> for Option<T> {
    fn from(opt: COption<T>) -> Self {
        match opt {
            COption::None => None,
            COption::Some(t) => Some(t),
        }
    }
}

impl<T> COption<T> {
    pub fn is_some(&self) -> bool {
        matches!(*self, COption::Some(_))
    }

    pub fn unwrap(self) -> T {
        match self {
            COption::Some(val) => val,
            COption::None => panic!("called `COption::unwrap()` on a `None` value"),
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        match *self {
            COption::Some(ref x) => Some(x),
            COption::None => None,
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        match *self {
            COption::Some(ref mut x) => Some(x),
            COption::None => None,
        }
    }

    pub fn take(&mut self) -> Option<T> {
        core::mem::take(self).into()
    }
}

#[cfg(feature = "serde")]
use core::fmt;
#[cfg(feature = "serde")]
use core::marker::PhantomData;
#[cfg(feature = "serde")]
use serde::{de, ser, Deserialize, Serialize};

#[cfg(feature = "serde")]
struct COptionVisitor<T> {
    marker: PhantomData<T>,
}

#[cfg(feature = "serde")]
impl<'de, T> de::Visitor<'de> for COptionVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = COption<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("option")
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(COption::None)
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(COption::None)
    }

    #[inline]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(COption::Some)
    }

    /*#[doc(hidden)]
    fn __private_visit_untagged_option<D>(self, deserializer: D) -> Result<Self::Value, ()>
    where
        D: Deserializer<'de>,
    {
        Ok(T::deserialize(deserializer).ok())
    }*/
}

#[cfg(feature = "serde")]
impl<'de, T> Deserialize<'de> for COption<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_option(COptionVisitor {
            marker: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
impl<T> Serialize for COption<T>
where
    T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            COption::Some(ref value) => serializer.serialize_some(value),
            COption::None => serializer.serialize_none(),
        }
    }
}
