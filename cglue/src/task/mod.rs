//! C-compatible task structures.

#[cfg(not(feature = "task_unsound"))]
mod sound;
#[cfg(not(feature = "task_unsound"))]
pub use sound::*;

#[cfg(feature = "task_unsound")]
mod unsound;
#[cfg(feature = "task_unsound")]
pub use unsound::*;

use core::task::*;

/// Actual type: unsafe fn(_: *const ()) -> RawWaker;
///
/// However, we don't want to expose it since it's not ABI safe.
#[cfg(not(feature = "task_unsound"))]
type CloneFn = *const ();
#[cfg(feature = "task_unsound")]
type CloneFn = unsafe fn(_: *const ()) -> RawWaker;

/// Actual type: unsafe fn(_: *const ());
///
/// However, we do not want to expose it since it's not ABI safe.
#[cfg(not(feature = "task_unsound"))]
type OtherFn = *const ();
#[cfg(feature = "task_unsound")]
type OtherFn = unsafe fn(_: *const ());

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[cfg_attr(
    all(feature = "abi_stable", feature = "task_unsound"),
    sabi(unsafe_opaque_fields)
)]
pub struct CRawWakerVTable {
    clone: CloneFn,
    wake: OtherFn,
    wake_by_ref: OtherFn,
    drop: OtherFn,
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CRawWaker {
    data: *const (),
    vtable: &'static CRawWakerVTable,
}

impl From<&Waker> for &CRawWaker {
    fn from(w: &Waker) -> Self {
        unsafe { &*(w as *const Waker as *const CRawWaker) }
    }
}

// Verify the layouts of reimplemented data structures
//
// Unfortunately, we cannot verify function ABI.
#[allow(clippy::useless_transmute)]
#[cfg(not(miri))]
const _: () = {
    use core::mem::{size_of, transmute};

    if size_of::<CRawWaker>() != size_of::<RawWaker>() {
        panic!("Raw waker size mismatch")
    }

    if size_of::<CRawWaker>() != size_of::<Waker>() {
        panic!("Raw waker size mismatch")
    }

    if size_of::<CRawWakerVTable>() != size_of::<RawWakerVTable>() {
        panic!("Raw waker vtbl size mismatch")
    }

    macro_rules! comp_arr {
        ($a:ident, $b:ident) => {
            let mut cnt = 0;
            while cnt < $a.len() {
                if $a[cnt] != $b[cnt] {
                    panic!("buffers not equal!");
                }
                cnt += 1;
            }
        };
    }

    // Verify the layout of the vtable.

    let clone: CloneFn = unsafe { transmute(1usize) };
    let wake: OtherFn = unsafe { transmute(2usize) };
    let wake_by_ref: OtherFn = unsafe { transmute(3usize) };
    let drop: OtherFn = unsafe { transmute(4usize) };

    let vtbl = unsafe {
        RawWakerVTable::new(
            transmute(clone),
            transmute(wake),
            transmute(wake_by_ref),
            transmute(drop),
        )
    };
    let vtbl_c = CRawWakerVTable {
        clone,
        wake,
        wake_by_ref,
        drop,
    };

    let bvtbl = unsafe { transmute::<_, [u8; size_of::<RawWakerVTable>()]>(vtbl) };
    let bvtbl_c = unsafe { transmute::<_, [u8; size_of::<RawWakerVTable>()]>(vtbl_c) };

    comp_arr!(bvtbl, bvtbl_c);

    // Verify the layout of the raw waker.

    let data = 10usize as *const ();
    let vtbl: &'static RawWakerVTable = unsafe { transmute(20usize) };
    let vtbl_c: &'static CRawWakerVTable = unsafe { transmute(20usize) };

    let waker = RawWaker::new(data, vtbl);
    let waker_c = CRawWaker {
        data,
        vtable: vtbl_c,
    };

    let bwaker = unsafe { transmute::<_, [u8; size_of::<RawWaker>()]>(waker) };
    let bwaker_c = unsafe { transmute::<_, [u8; size_of::<RawWaker>()]>(waker_c) };

    comp_arr!(bwaker, bwaker_c);
};
