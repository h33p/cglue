/// Describes an FFI-safe wrapped box.
use core::ops::{Deref, DerefMut};

/// FFI-safe box
///
/// This box has a static self reference, alongside a custom drop function.
///
/// The drop function can be called from anywhere, it will free on correct allocator internally.
#[repr(C)]
pub struct CBox<T: 'static> {
    instance: &'static mut T,
    drop: unsafe extern "C" fn(&mut T),
}

impl<T> Deref for CBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.instance
    }
}

impl<T> DerefMut for CBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.instance
    }
}

impl<T> From<Box<T>> for CBox<T> {
    fn from(this: Box<T>) -> Self {
        let instance = Box::leak(this);
        Self {
            instance,
            drop: cglue_drop_box::<T>,
        }
    }
}

impl<T> From<T> for CBox<T> {
    fn from(this: T) -> Self {
        CBox::from(Box::new(this))
    }
}

impl<T> Drop for CBox<T> {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.instance) };
    }
}

unsafe extern "C" fn cglue_drop_box<T>(this: &mut T) {
    let _ = Box::from_raw(this);
}
