use super::*;

use core::cell::UnsafeCell;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::*;
use tarc::BaseArc;

unsafe extern "C" fn clone_adapter(data: *const (), clone: *const ()) -> CRawWaker {
    let clone: unsafe fn(*const ()) -> CRawWaker = core::mem::transmute(clone);
    clone(data)
}

unsafe extern "C" fn other_adapter(data: *const (), other: *const ()) {
    let other: unsafe fn(_: *const ()) = core::mem::transmute(other);
    other(data)
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CWaker {
    raw: CRawWaker,
    clone_adapter: unsafe extern "C" fn(*const (), *const ()) -> CRawWaker,
    other_adapter: unsafe extern "C" fn(*const (), *const ()),
}

impl CWaker {
    pub fn into_waker(this: BaseArc<Self>) -> Waker {
        unsafe { Waker::from_raw(Self::into_raw_waker(this)) }
    }

    pub fn into_raw_waker(this: BaseArc<Self>) -> RawWaker {
        unsafe fn clone(data: *const ()) -> RawWaker {
            let waker = data as *const CWaker;
            BaseArc::increment_strong_count(waker);
            CWaker::into_raw_waker(BaseArc::from_raw(waker))
        }

        unsafe fn wake(data: *const ()) {
            let waker = &*(data as *const CWaker);
            (waker.other_adapter)(waker.raw.data, waker.raw.vtable.wake_by_ref as _);
            BaseArc::decrement_strong_count(waker);
        }

        unsafe fn wake_by_ref(data: *const ()) {
            let waker = &*(data as *const CWaker);
            (waker.other_adapter)(waker.raw.data, waker.raw.vtable.wake_by_ref as _);
        }

        unsafe fn drop(data: *const ()) {
            BaseArc::decrement_strong_count(data as *const CWaker);
        }

        let vtbl = &RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        RawWaker::new(this.into_raw() as *const (), vtbl)
    }

    pub fn wake(self) {
        let other_adapter = self.other_adapter;
        let wake = self.raw.vtable.wake;
        let data = self.raw.data;
        // Don't call `drop` -- the waker will be consumed by `wake`.
        core::mem::forget(self);
        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.

        // This is also FFI-safe because `other_adapter` adapts the calling convention.
        unsafe { (other_adapter)(data, wake as _) };
    }

    pub fn wake_by_ref(&self) {
        let other_adapter = self.other_adapter;
        let wake_by_ref = self.raw.vtable.wake_by_ref;
        let data = self.raw.data;
        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.

        // This is also FFI-safe because `other_adapter` adapts the calling convention.
        unsafe { (other_adapter)(data, wake_by_ref as _) };
    }

    pub unsafe fn from_raw(raw: RawWaker) -> Self {
        let raw: CRawWaker = core::mem::transmute(raw);

        Self {
            raw,
            clone_adapter,
            other_adapter,
        }
    }
}

impl Drop for CWaker {
    fn drop(&mut self) {
        let other_adapter = self.other_adapter;
        let drop = self.raw.vtable.drop;
        let data = self.raw.data;
        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.

        // This is also FFI-safe because `other_adapter` adapts the calling convention.
        unsafe { (other_adapter)(data, drop as *const ()) };
    }
}

impl Clone for CWaker {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            // SAFETY: This is safe because `Waker::from_raw` is the only way
            // to initialize `clone` and `data` requiring the user to acknowledge
            // that the contract of [`RawWaker`] is upheld.
            raw: unsafe { (self.clone_adapter)(self.raw.data, self.raw.vtable.clone as *const ()) },
            clone_adapter: self.clone_adapter,
            other_adapter: self.other_adapter,
        }
    }
}

impl From<Waker> for CWaker {
    fn from(waker: Waker) -> Self {
        unsafe { Self::from_raw(core::mem::transmute(waker)) }
    }
}

#[repr(C, u8)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
enum FastCWakerState<'a> {
    Borrowed {
        waker: &'a CRawWaker,
        clone_adapter: unsafe extern "C" fn(*const (), *const ()) -> CRawWaker,
        other_adapter: unsafe extern "C" fn(*const (), *const ()),
    },
    Owned(NonNull<CWaker>),
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct FastCWaker<'a> {
    lock: AtomicBool,
    state: UnsafeCell<FastCWakerState<'a>>,
}

impl<'a> FastCWaker<'a> {
    #[allow(clippy::mut_from_ref)]
    unsafe fn lock(&self) -> &mut FastCWakerState<'a> {
        while self.lock.fetch_or(true, Ordering::Acquire) {
            while self.lock.load(Ordering::Relaxed) {
                #[allow(deprecated)]
                core::sync::atomic::spin_loop_hint();
            }
        }
        unsafe { &mut *self.state.get() }
    }

    fn release(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

impl<'a> Drop for FastCWakerState<'a> {
    fn drop(&mut self) {
        if let Self::Owned(s) = self {
            unsafe { BaseArc::decrement_strong_count(s) };
        }
    }
}

impl<'a> From<&'a Waker> for FastCWaker<'a> {
    fn from(waker: &'a Waker) -> Self {
        Self {
            lock: Default::default(),
            state: UnsafeCell::new(FastCWakerState::Borrowed {
                waker: waker.into(),
                clone_adapter,
                other_adapter,
            }),
        }
    }
}

impl<'a> FastCWaker<'a> {
    pub fn with_waker<T>(&self, cb: impl FnOnce(&Waker) -> T) -> T {
        if let FastCWakerState::Owned(owned) = unsafe { self.lock() } {
            let owned = unsafe {
                let owned = owned.as_ptr();
                BaseArc::increment_strong_count(owned);
                BaseArc::from_raw(owned)
            };
            self.release();
            cb(&CWaker::into_waker(owned))
        } else {
            self.release();
            // Create a waker that modifies self upon clone

            unsafe fn unreach(_: *const ()) {
                unreachable!()
            }
            unsafe fn clone(data: *const ()) -> RawWaker {
                let this = &*(data as *const FastCWaker);
                CWaker::into_raw_waker(this.c_waker())
            }
            unsafe fn wake_by_ref(data: *const ()) {
                let this = &*(data as *const FastCWaker);
                let (data, wake, adapter) = match unsafe { this.lock() } {
                    FastCWakerState::Borrowed {
                        waker,
                        other_adapter,
                        ..
                    } => (waker.data, waker.vtable.wake_by_ref, *other_adapter),
                    FastCWakerState::Owned(d) => {
                        let waker = &*d.as_ptr();
                        (
                            waker.raw.data,
                            waker.raw.vtable.wake_by_ref,
                            waker.other_adapter,
                        )
                    }
                };
                this.release();

                adapter(data, wake as _)
            }

            let vtbl = &RawWakerVTable::new(clone, unreach, wake_by_ref, unreach);
            let waker = RawWaker::new(self as *const Self as *const (), vtbl);
            let waker = unsafe { Waker::from_raw(waker) };

            cb(&waker)
        }
    }

    pub fn clone_waker(&self) -> Waker {
        self.with_waker(|w| w.clone())
    }

    pub fn c_waker(&self) -> BaseArc<CWaker> {
        let state = unsafe { self.lock() };

        let ret = match state {
            FastCWakerState::Owned(owned) => unsafe {
                let owned = owned.as_ptr();
                BaseArc::increment_strong_count(owned);
                BaseArc::from_raw(owned)
            },
            FastCWakerState::Borrowed {
                waker,
                clone_adapter,
                other_adapter,
            } => {
                let waker = unsafe { clone_adapter(waker.data, waker.vtable.clone as _) };
                let owned: BaseArc<CWaker> = BaseArc::from(CWaker {
                    raw: waker,
                    clone_adapter: *clone_adapter,
                    other_adapter: *other_adapter,
                });
                *state = FastCWakerState::Owned(unsafe {
                    NonNull::new_unchecked(owned.clone().into_raw() as *mut _)
                });
                owned
            }
        };

        self.release();

        ret
    }
}
