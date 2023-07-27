//! C-compatible task structures.

mod sound;
pub use sound::*;

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

#[repr(transparent)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
pub struct CRawWaker {
    ptrs: [*const (); 2],
}

impl From<&Waker> for &CRawWaker {
    fn from(w: &Waker) -> Self {
        unsafe { &*(w as *const Waker as *const CRawWaker) }
    }
}

impl CRawWaker {
    fn data(&self, order: &CRawWakerOrder) -> *const () {
        self.ptrs[order.data]
    }

    unsafe fn vtable(&self, order: &CRawWakerOrder) -> &'static CRawWakerVTable {
        &*(self.ptrs[order.vtable] as *const CRawWakerVTable)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CRawWakerOrder {
    data: usize,
    vtable: usize,
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

    macro_rules! expand {
        ($c:literal, $s:literal, $($e:expr),*) => {
            [$(concat!($c, " ", $s, " idx ", stringify!($e)),)*]
        }
    }

    macro_rules! comp_arr {
        ($a:ident, $b:ident, $c:literal) => {{
            const BUF: &[&str] = &expand!(
                $c,
                "buffers not equal!",
                0,
                1,
                2,
                3,
                4,
                5,
                6,
                7,
                8,
                9,
                10,
                11,
                12,
                13,
                14,
                15,
                16,
                17,
                18,
                19,
                20,
                21,
                22,
                23,
                24,
                25,
                26,
                27,
                28,
                29,
                30,
                31,
                32
            );

            let mut cnt = 0;
            while cnt < $a.len() {
                if $a[cnt] != $b[cnt] {
                    panic!("{}", BUF[cnt]);
                }
                cnt += 1;
            }
        }};
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

    comp_arr!(bvtbl, bvtbl_c, "vtable");
};

macro_rules! __order {
    () => {{
        use core::mem::transmute;

        // Verify the layout of the raw waker.

        let data = core::ptr::null();

        #[cfg(miri)]
        let vtbl = unsafe {
            unsafe fn clone(data: *const ()) -> RawWaker {
                todo!()
            }

            unsafe fn null(data: *const ()) {}

            &RawWakerVTable::new(clone, null, null, null)
        };
        #[cfg(not(miri))]
        let vtbl = unsafe { transmute(1 as *const ()) };

        let waker = RawWaker::new(data, vtbl);

        // This verifies the size of the object - will not compile if RawWaker is not the size of 2
        // pointers, or if usize != pointer size.
        let bwaker = unsafe { transmute::<_, [usize; 2]>(waker) };

        // This will return us the order
        let data = if bwaker[0] == 0 { 0 } else { 1 };
        let vtable = if bwaker[0] == 0 { 1 } else { 0 };

        CRawWakerOrder { data, vtable }
    }};
}

#[cfg(miri)]
#[allow(clippy::useless_transmute)]
fn get_order() -> CRawWakerOrder {
    __order!()
}

#[allow(clippy::useless_transmute)]
#[cfg(not(miri))]
const ORDER: CRawWakerOrder = { __order!() };

#[cfg(not(miri))]
const fn get_order() -> CRawWakerOrder {
    ORDER
}
