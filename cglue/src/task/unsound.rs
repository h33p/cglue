use super::*;

use core::task::*;
use tarc::BaseArc;

#[repr(transparent)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CWaker {
    raw: CRawWaker,
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
            (waker.raw.vtable.wake_by_ref)(waker.raw.data);
            BaseArc::decrement_strong_count(waker);
        }

        unsafe fn wake_by_ref(data: *const ()) {
            let waker = &*(data as *const CWaker);
            (waker.raw.vtable.wake_by_ref)(waker.raw.data);
        }

        unsafe fn drop(data: *const ()) {
            BaseArc::decrement_strong_count(data as *const CWaker);
        }

        let vtbl = &RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        RawWaker::new(this.into_raw() as *const (), vtbl)
    }

    pub fn wake(self) {
        let wake = self.raw.vtable.wake;
        let data = self.raw.data;
        // Don't call `drop` -- the waker will be consumed by `wake`.
        core::mem::forget(self);
        // SAFETY: This is somewhat safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.
        // NOTE: function ABI is unverified.
        unsafe { wake(data) };
    }

    pub fn wake_by_ref(&self) {
        let wake_by_ref = self.raw.vtable.wake_by_ref;
        let data = self.raw.data;
        // SAFETY: This is somewhat safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.
        // NOTE: function ABI is unverified.
        unsafe { wake_by_ref(data) };
    }

    pub unsafe fn from_raw(raw: RawWaker) -> Self {
        let raw: CRawWaker = core::mem::transmute(raw);

        Self { raw }
    }
}

impl Drop for CWaker {
    fn drop(&mut self) {
        let data = self.raw.data;
        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.
        // NOTE: function ABI is unverified.
        unsafe { (self.raw.vtable.drop)(data) };
    }
}

impl Clone for CWaker {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            // SAFETY: This is safe because `Waker::from_raw` is the only way
            // to initialize `clone` and `data` requiring the user to acknowledge
            // that the contract of [`RawWaker`] is upheld.
            // NOTE: function ABI is unverified.
            raw: unsafe { core::mem::transmute((self.raw.vtable.clone)(self.raw.data)) },
        }
    }
}

impl From<Waker> for CWaker {
    fn from(waker: Waker) -> Self {
        unsafe { Self::from_raw(core::mem::transmute(waker)) }
    }
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct FastCWaker<'a> {
    waker: &'a CWaker,
}

impl<'a> From<&'a Waker> for FastCWaker<'a> {
    fn from(waker: &'a Waker) -> Self {
        Self {
            waker: unsafe { &*(waker as *const Waker as *const CWaker) },
        }
    }
}

impl<'a> FastCWaker<'a> {
    pub fn with_waker<T>(&self, cb: impl FnOnce(&Waker) -> T) -> T {
        let waker = unsafe { &*(self.waker as *const CWaker as *const Waker) };
        cb(waker)
    }

    pub fn clone_waker(&self) -> Waker {
        self.with_waker(|w| w.clone())
    }
}
