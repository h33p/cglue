//! C-compatible task structures.

mod sound;
pub use sound::*;

use core::task::*;

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CRawWakerVTable {
    clone: *const (),
    wake: *const (),
    wake_by_ref: *const (),
    drop: *const (),
}

impl<'a> From<&'a CRawWakerVTable> for &'a RawWakerVTable {
    fn from(vtbl: &'a CRawWakerVTable) -> Self {
        unsafe { core::mem::transmute(vtbl) }
    }
}

impl From<CRawWakerVTable> for RawWakerVTable {
    fn from(vtbl: CRawWakerVTable) -> Self {
        unsafe { core::mem::transmute(vtbl) }
    }
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

    fn to_c(self, order: &CRawWakerOrder) -> Self {
        Self {
            ptrs: [self.ptrs[order.data], self.ptrs[order.vtable]],
        }
    }
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
struct CRawWakerOrder {
    data: usize,
    vtable: usize,
}

impl CRawWakerOrder {
    fn c_order() -> Self {
        Self { data: 0, vtable: 1 }
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
        #[cfg(not(const_panic_on_stable))]
        let _ = "Raw waker size mismatch".as_bytes()[!0];
        #[cfg(const_panic_on_stable)]
        panic!("Raw waker size mismatch");
    }

    if size_of::<CRawWaker>() != size_of::<Waker>() {
        #[cfg(not(const_panic_on_stable))]
        let _ = "Raw waker size mismatch".as_bytes()[!0];
        #[cfg(const_panic_on_stable)]
        panic!("Raw waker size mismatch");
    }

    if size_of::<CRawWakerVTable>() != size_of::<RawWakerVTable>() {
        #[cfg(not(const_panic_on_stable))]
        let _ = "Raw waker vtbl size mismatch".as_bytes()[!0];
        #[cfg(const_panic_on_stable)]
        panic!("Raw waker vtbl size mismatch");
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
                    #[cfg(not(const_panic_on_stable))]
                    let _buffers_not_equal: () = [][cnt];
                    #[cfg(const_panic_on_stable)]
                    panic!("{}", BUF[cnt]);
                }
                cnt += 1;
            }
        }};
    }

    // Verify the layout of the vtable.

    let clone: *const () = unsafe { transmute(1usize) };
    let wake: *const () = unsafe { transmute(2usize) };
    let wake_by_ref: *const () = unsafe { transmute(3usize) };
    let drop: *const () = unsafe { transmute(4usize) };

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
#[allow(unused_braces)]
#[cfg(not(miri))]
const ORDER: CRawWakerOrder = { __order!() };

#[cfg(not(miri))]
const fn get_order() -> CRawWakerOrder {
    ORDER
}
