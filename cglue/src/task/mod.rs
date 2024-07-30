//! C-compatible task structures.

use core::task::*;
use tarc::BaseArc;

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
struct OpaqueRawWakerVtbl {
    clone: unsafe extern "C" fn(OpaqueRawWaker) -> CRawWaker,
    wake: unsafe extern "C" fn(OpaqueRawWaker),
    wake_by_ref: unsafe extern "C" fn(OpaqueRawWaker),
    drop: unsafe extern "C" fn(OpaqueRawWaker),
}

impl Default for &'static OpaqueRawWakerVtbl {
    fn default() -> Self {
        unsafe extern "C" fn clone(w: OpaqueRawWaker) -> CRawWaker {
            let waker: RawWaker = core::mem::transmute(w);
            waker_clone(&waker as *const _ as *const ())
        }

        unsafe extern "C" fn wake(w: OpaqueRawWaker) {
            let waker: Waker = core::mem::transmute(w);
            waker.wake()
        }

        unsafe extern "C" fn wake_by_ref(w: OpaqueRawWaker) {
            let waker: RawWaker = core::mem::transmute(w);
            let waker: &Waker = core::mem::transmute(&waker);
            waker.wake_by_ref()
        }

        unsafe extern "C" fn drop(w: OpaqueRawWaker) {
            let _: Waker = core::mem::transmute(w);
        }

        &OpaqueRawWakerVtbl {
            clone,
            wake,
            wake_by_ref,
            drop,
        }
    }
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
struct CRawWaker {
    waker: OpaqueRawWaker,
    vtable: &'static OpaqueRawWakerVtbl,
}

impl CRawWaker {
    fn to_raw(this: BaseArc<CRawWaker>) -> RawWaker {
        unsafe fn clone(data: *const ()) -> RawWaker {
            let data = data as *const CRawWaker;
            BaseArc::increment_strong_count(data);
            let waker = BaseArc::from_raw(data);
            CRawWaker::to_raw(waker)
        }
        unsafe fn wake(data: *const ()) {
            let this = BaseArc::from_raw(data as *const CRawWaker);
            (this.vtable.wake)(this.waker)
        }
        unsafe fn wake_by_ref(data: *const ()) {
            let data = data as *const CRawWaker;
            let this = &*data;
            (this.vtable.wake_by_ref)(this.waker)
        }
        unsafe fn drop(data: *const ()) {
            let this = BaseArc::from_raw(data as *const CRawWaker);
            (this.vtable.drop)(this.waker)
        }

        let vtbl = &RawWakerVTable::new(clone, wake, wake_by_ref, drop);

        RawWaker::new(this.into_raw() as *const (), vtbl)
    }
}

unsafe extern "C" fn waker_clone(waker: *const ()) -> CRawWaker {
    let waker: &Waker = &*(waker as *const Waker);
    let waker = core::mem::transmute(waker.clone());

    CRawWaker {
        waker,
        vtable: Default::default(),
    }
}

unsafe extern "C" fn waker_wake_by_ref(waker: *const ()) {
    let waker: &Waker = &*(waker as *const Waker);
    waker.wake_by_ref()
}

#[repr(transparent)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
struct OpaqueRawWaker {
    waker: [*const (); 2],
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
pub struct CRefWaker<'a> {
    raw: &'a OpaqueRawWaker,
    clone: unsafe extern "C" fn(*const ()) -> CRawWaker,
    wake_by_ref: unsafe extern "C" fn(*const ()),
}

impl<'a> CRefWaker<'a> {
    pub unsafe fn from_raw(raw: &'a RawWaker) -> Self {
        let raw: &'a OpaqueRawWaker = core::mem::transmute(raw);

        Self {
            raw,
            clone: waker_clone,
            wake_by_ref: waker_wake_by_ref,
        }
    }

    pub fn with_waker<T>(&self, cb: impl FnOnce(&Waker) -> T) -> T {
        unsafe fn unreach(_: *const ()) {
            unreachable!()
        }
        unsafe fn noop(_: *const ()) {}
        unsafe fn clone(data: *const ()) -> RawWaker {
            let this = &*(data as *const CRefWaker);
            let waker = unsafe { (this.clone)(this.raw as *const _ as *const ()) };
            let waker = BaseArc::new(waker);
            CRawWaker::to_raw(waker)
        }
        unsafe fn wake_by_ref(data: *const ()) {
            let this = &*(data as *const CRefWaker);
            unsafe { (this.wake_by_ref)(this.raw as *const _ as *const ()) };
        }

        let vtbl = &RawWakerVTable::new(clone, unreach, wake_by_ref, noop);
        let waker = RawWaker::new(self as *const Self as *const (), vtbl);
        let waker = unsafe { Waker::from_raw(waker) };

        cb(&waker)
    }
}

impl<'a> From<&'a Waker> for CRefWaker<'a> {
    fn from(waker: &'a Waker) -> Self {
        const _: [(); core::mem::size_of::<Waker>()] = [(); core::mem::size_of::<OpaqueRawWaker>()];
        unsafe { Self::from_raw(core::mem::transmute(waker)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pollster::block_on;

    // Since unavailable before 1.64
    use core::fmt;
    use core::future::Future;
    use core::pin::*;

    pub fn poll_fn<T, F>(f: F) -> PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        PollFn { f }
    }

    /// A Future that wraps a function returning [`Poll`].
    ///
    /// This `struct` is created by [`poll_fn()`]. See its
    /// documentation for more.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollFn<F> {
        f: F,
    }

    impl<F: Unpin> Unpin for PollFn<F> {}

    impl<F> fmt::Debug for PollFn<F> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PollFn").finish()
        }
    }

    impl<T, F> Future for PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        type Output = T;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            // SAFETY: We are not moving out of the pinned field.
            (unsafe { &mut self.get_unchecked_mut().f })(cx)
        }
    }

    #[test]
    fn cwaker_simple() {
        let mut polled = false;
        let fut = poll_fn(|cx| {
            if !polled {
                polled = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        });
        let fut = crate::trait_obj!(fut as Future);
        block_on(fut)
    }

    #[test]
    fn cwaker_simple_cloned() {
        let mut polled = false;
        let fut = poll_fn(|cx| {
            if !polled {
                polled = true;
                cx.waker().clone().wake();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        });
        let fut = crate::trait_obj!(fut as Future);
        block_on(fut)
    }

    #[test]
    fn cwaker_threaded() {
        let (tx, rx) = std::sync::mpsc::channel::<Waker>();

        let thread = std::thread::spawn(move || {
            for waker in rx.into_iter() {
                waker.wake();
            }
        });

        let mut polled = false;
        let fut = poll_fn(move |cx| {
            if !polled {
                polled = true;
                tx.send(cx.waker().clone()).unwrap();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        });
        let fut = crate::trait_obj!(fut as Future);
        block_on(fut);

        thread.join().unwrap();
    }
}
