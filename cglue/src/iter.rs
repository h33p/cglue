//! # FFI compatible iterators
//!
//! It is a simple interface that allows to pass streams into functions.

use crate::trait_group::c_void;
use core::mem::MaybeUninit;
use std::prelude::v1::*;

/// FFI compatible iterator.
///
/// Any mutable reference to an iterator can be converted to a `CIterator`.
///
/// `CIterator<T>` implements `Iterator<Item = T>`.
///
/// # Examples
///
/// Using [`AsCIterator`](AsCIterator) helper:
///
/// ```
/// use cglue::iter::{CIterator, AsCIterator};
///
/// extern "C" fn sum_all(iter: CIterator<usize>) -> usize {
///     iter.sum()
/// }
///
/// let mut iter = (0..10).map(|v| v * v);
///
/// assert_eq!(sum_all(iter.as_citer()), 285);
/// ```
///
/// Converting with `Into` trait:
///
/// ```
/// use cglue::iter::{CIterator, AsCIterator};
///
/// extern "C" fn sum_all(iter: CIterator<usize>) -> usize {
///     iter.sum()
/// }
///
/// let mut iter = (0..=10).map(|v| v * v);
///
/// assert_eq!(sum_all((&mut iter).into()), 385);
/// ```
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CIterator<'a, T> {
    iter: &'a mut c_void,
    func: extern "C" fn(&mut c_void, out: &mut MaybeUninit<T>) -> i32,
}

impl<'a, I: Iterator<Item = T>, T> From<&'a mut I> for CIterator<'a, T> {
    fn from(iter: &'a mut I) -> Self {
        CIterator::new(iter)
    }
}

impl<'a, T> CIterator<'a, T> {
    pub fn new<I: Iterator<Item = T>>(iter: &'a mut I) -> Self {
        extern "C" fn func<I: Iterator<Item = T>, T>(
            iter: &mut I,
            out: &mut MaybeUninit<T>,
        ) -> i32 {
            match iter.next() {
                Some(e) => {
                    unsafe { out.as_mut_ptr().write(e) };
                    0
                }
                None => 1,
            }
        }

        // SAFETY: type erasure is safe here, because the values are encapsulated and always in
        // a pair.
        let iter = unsafe { (iter as *mut _ as *mut c_void).as_mut().unwrap() };
        let func = func::<I, T> as extern "C" fn(_, _) -> _;
        let func = unsafe { std::mem::transmute::<_, _>(func) };

        Self { iter, func }
    }
}

impl<'a, T> Iterator for CIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = MaybeUninit::uninit();
        if (self.func)(self.iter, &mut out) == 0 {
            Some(unsafe { out.assume_init() })
        } else {
            None
        }
    }
}

pub trait AsCIterator: Iterator + Sized {
    fn as_citer(&mut self) -> CIterator<Self::Item> {
        self.into()
    }
}

impl<T: Iterator> AsCIterator for T {}
