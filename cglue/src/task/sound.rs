use super::*;

use core::task::*;

unsafe extern "C" fn waker_clone(data: *const ()) -> CRawWaker {
    let waker: &Waker = &*(data as *const Waker);
    let waker: CRawWaker = core::mem::transmute(waker.clone());
    waker.to_c(&get_order())
}

unsafe extern "C" fn waker_wake_by_ref(data: *const ()) {
    let waker: &Waker = &*(data as *const Waker);
    waker.wake_by_ref()
}

#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
#[derive(Clone, Copy)]
pub struct CWaker<'a> {
    raw: &'a CRawWaker,
    clone: unsafe extern "C" fn(*const ()) -> CRawWaker,
    wake_by_ref: unsafe extern "C" fn(*const ()),
}

impl<'a> CWaker<'a> {
    pub unsafe fn from_raw(raw: &'a RawWaker) -> Self {
        let raw: &'a CRawWaker = core::mem::transmute(raw);

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
            let this = &*(data as *const CWaker);
            let waker = unsafe { (this.clone)(this.raw as *const _ as *const ()) };
            RawWaker::new(
                waker.data(&CRawWakerOrder::c_order()),
                waker.vtable(&CRawWakerOrder::c_order()).into(),
            )
        }
        unsafe fn wake_by_ref(data: *const ()) {
            let this = &*(data as *const CWaker);
            unsafe { (this.wake_by_ref)(this.raw as *const _ as *const ()) };
        }

        let vtbl = &RawWakerVTable::new(clone, unreach, wake_by_ref, noop);
        let waker = RawWaker::new(self as *const Self as *const (), vtbl);
        let waker = unsafe { Waker::from_raw(waker) };

        cb(&waker)
    }
}

impl<'a> From<&'a Waker> for CWaker<'a> {
    fn from(waker: &'a Waker) -> Self {
        const _: [(); core::mem::size_of::<Waker>()] = [(); core::mem::size_of::<CRawWaker>()];
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
        let fut = poll_fn(|cx| {
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

        core::mem::drop(tx);

        thread.join().unwrap();
    }
}
