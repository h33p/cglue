//! # FFI safe result.
//!
//! This module contains several key parts:
//!
//! ## [CResult](crate::result::CResult)
//!
//! It is a simple `#[repr(C)]` enum that is equivalent and interchangeable with `Result`.
//!
//! ## [IntError](crate::result::IntError)
//!
//! [IntError](crate::result::IntError) is a type that allows for efficient FFI-boundary crossing
//! and simple interop with C code. It takes a `Result<T, E>`, and splits it up to 2 distinct parts
//! - `ok_out` pointer, and an integer return value. Value of zero always means success, and that
//! `ok_out` was filled, whereas any other value can represent a specific meaning `E` must specify
//! by itself.
//!
//! ## [IntResult](crate::result::IntResult)
//!
//! It is a helper trait that is implemented on all `Result<T, E>` types where `E` implements
//! [IntError](crate::result::IntError).
//!
use core::mem::MaybeUninit;
use core::num::NonZeroI32;

/// FFI safe result.
///
/// This type is not meant for general use, but rather as a last-resort conversion for type wrapping.
///
/// Typical workflow would include temporarily converting into/from CResult.
///
/// But preferred way to pass results efficiently would be to implement `IntError` trait on the `E`
/// type.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub enum CResult<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> From<Result<T, E>> for CResult<T, E> {
    fn from(opt: Result<T, E>) -> Self {
        match opt {
            Ok(t) => Self::Ok(t),
            Err(e) => Self::Err(e),
        }
    }
}

impl<T, E> From<CResult<T, E>> for Result<T, E> {
    fn from(opt: CResult<T, E>) -> Self {
        match opt {
            CResult::Ok(t) => Ok(t),
            CResult::Err(e) => Err(e),
        }
    }
}

impl<T, E> CResult<T, E> {
    pub fn is_ok(&self) -> bool {
        matches!(*self, CResult::Ok(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(*self, CResult::Err(_))
    }

    pub fn unwrap(self) -> T
    where
        E: core::fmt::Debug,
    {
        Result::from(self).unwrap()
    }

    pub fn ok(self) -> Option<T> {
        match self {
            CResult::Ok(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Result<&T, &E> {
        match *self {
            CResult::Ok(ref x) => Ok(x),
            CResult::Err(ref e) => Err(e),
        }
    }

    pub fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match *self {
            CResult::Ok(ref mut x) => Ok(x),
            CResult::Err(ref mut e) => Err(e),
        }
    }
}

/// Helper trait for integer errors.
///
/// This trait essentially forwards [`into_int_result`](crate::result::into_int_result), and
/// [`into_int_out_result`](crate::result::into_int_out_result) functions for easier access.
pub trait IntResult<T> {
    fn into_int_result(self) -> i32;
    fn into_int_out_result(self, ok_out: &mut MaybeUninit<T>) -> i32;
}

impl<T, E: IntError> IntResult<T> for Result<T, E> {
    fn into_int_result(self) -> i32 {
        into_int_result(self)
    }

    fn into_int_out_result(self, ok_out: &mut MaybeUninit<T>) -> i32 {
        into_int_out_result(self, ok_out)
    }
}

/// Represents an integer-convertable error value.
///
/// This trait can be implemented for error types to allow for more
/// efficient conversion and more user-friendly usage from C API side.
pub trait IntError {
    fn into_int_err(self) -> NonZeroI32;
    fn from_int_err(err: NonZeroI32) -> Self;
}

#[cfg(feature = "std")]
impl IntError for std::io::Error {
    fn into_int_err(self) -> NonZeroI32 {
        let err = self.raw_os_error().unwrap_or(0);

        let err = if err == 0 {
            // TODO: match numbers here for io::ErrorKind
            0xffff
        } else {
            err
        };

        NonZeroI32::new(err).unwrap()
    }

    fn from_int_err(err: NonZeroI32) -> Self {
        Self::from_raw_os_error(err.get())
    }
}

impl IntError for () {
    fn into_int_err(self) -> NonZeroI32 {
        NonZeroI32::new(1).unwrap()
    }

    fn from_int_err(_err: NonZeroI32) -> Self {}
}

impl IntError for core::fmt::Error {
    fn into_int_err(self) -> NonZeroI32 {
        NonZeroI32::new(1).unwrap()
    }

    fn from_int_err(_err: NonZeroI32) -> Self {
        Self
    }
}

/// Convert result into an integer error value.
///
/// Returned value of `0` means that the result is of Ok value, otherwise it is an error.
///
/// # Arguments
///
/// * `res` - result to convert.
pub fn into_int_result<T, E: IntError>(res: Result<T, E>) -> i32 {
    match res {
        Ok(_) => 0,
        Err(e) => e.into_int_err().get(),
    }
}

/// Convert result into an integer error value, potentially writing the Ok value.
///
/// If return value is `0`, `ok_out` will have initialised data, otherwise not.
///
/// # Arguments
///
/// * `res` - result to convert.
/// * `ok_out` - target output for Ok value.
pub fn into_int_out_result<T, E: IntError>(res: Result<T, E>, ok_out: &mut MaybeUninit<T>) -> i32 {
    match res {
        Ok(v) => {
            unsafe { ok_out.as_mut_ptr().write(v) };
            0
        }
        Err(e) => e.into_int_err().get(),
    }
}

/// Convert from error code to concrete result.
///
/// # Arguments
///
/// * `res` - result int value. Value of `0` means `Ok`.
/// * `ok_val` - Ok value to use, if result is `Ok`.
///
/// # Safety
///
/// `ok_val` must be initialised if `res = 0`. This can be used safely in conjunction with
/// `into_int_out_result`, assuming arguments are not modified in-between the calls.
pub unsafe fn from_int_result<T, E: IntError>(res: i32, ok_val: MaybeUninit<T>) -> Result<T, E> {
    match NonZeroI32::new(res) {
        None => Ok(ok_val.assume_init()),
        Some(e) => Err(E::from_int_err(e)),
    }
}

/// Convert from error code to Ok or Err.
///
/// # Arguments
///
/// * `res` - result int value. Value of `0` will return `Ok`.
pub fn from_int_result_empty<E: IntError>(res: i32) -> Result<(), E> {
    match NonZeroI32::new(res) {
        None => Ok(()),
        Some(e) => Err(E::from_int_err(e)),
    }
}
