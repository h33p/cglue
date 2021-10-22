//! Describes an FFI-safe Arc.
use crate::trait_group::Opaquable;
use std::ffi::c_void;
use std::sync::Arc;

/// FFI-Safe Arc
///
/// This Arc essentially uses clone/drop from the module that created it, to not mix up global
/// allocators.
#[repr(C)]
pub struct CArc<T: Sized + 'static> {
    instance: Option<&'static T>,
    clone_fn: unsafe extern "C" fn(Option<&'static T>) -> Option<&'static T>,
    drop_fn: unsafe extern "C" fn(Option<&T>),
}

impl<T> From<T> for CArc<T> {
    fn from(obj: T) -> Self {
        Self::from(Arc::new(obj))
    }
}

impl<T> From<Arc<T>> for CArc<T> {
    fn from(arc: Arc<T>) -> Self {
        Self {
            instance: unsafe { Arc::into_raw(arc).as_ref() },
            clone_fn: c_clone,
            drop_fn: c_drop,
        }
    }
}

impl<T> CArc<T> {
    pub fn into_opt(self) -> COptArc<T> {
        Some(self).into()
    }
}

unsafe impl<T: Sync + Send> Send for CArc<T> {}
unsafe impl<T: Sync + Send> Sync for CArc<T> {}

impl<T> Clone for CArc<T> {
    fn clone(&self) -> Self {
        Self {
            instance: unsafe { (self.clone_fn)(self.instance) },
            ..*self
        }
    }
}

impl<T> Drop for CArc<T> {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.instance) }
    }
}

impl<T> AsRef<T> for CArc<T> {
    fn as_ref(&self) -> &T {
        self.instance.unwrap()
    }
}

unsafe impl<T> Opaquable for CArc<T> {
    type OpaqueTarget = CArc<c_void>;
}

unsafe impl<T: Sync + Send> Send for COptArc<T> {}
unsafe impl<T: Sync + Send> Sync for COptArc<T> {}

#[repr(C)]
pub struct COptArc<T: Sized + 'static> {
    instance: Option<&'static T>,
    clone_fn: Option<unsafe extern "C" fn(Option<&'static T>) -> Option<&'static T>>,
    drop_fn: Option<unsafe extern "C" fn(Option<&T>)>,
}

impl<T> Clone for COptArc<T> {
    fn clone(&self) -> Self {
        match <Option<&CArc<T>>>::from(self) {
            Some(arc) => Some(arc.clone()).into(),
            None => None.into(),
        }
    }
}

impl<T> Drop for COptArc<T> {
    fn drop(&mut self) {
        if let Some(arc) = <Option<&mut CArc<T>>>::from(self) {
            unsafe { std::ptr::drop_in_place(arc) };
        }
    }
}

impl<T> COptArc<T> {
    pub fn take(&mut self) -> COptArc<T> {
        Self {
            instance: self.instance.take(),
            clone_fn: self.clone_fn.take(),
            drop_fn: self.drop_fn.take(),
        }
    }
}

impl<T> From<Option<CArc<T>>> for COptArc<T> {
    fn from(opt: Option<CArc<T>>) -> Self {
        match opt {
            Some(mut arc) => Self {
                instance: arc.instance.take(),
                clone_fn: Some(arc.clone_fn),
                drop_fn: Some(arc.drop_fn),
            },
            None => Self {
                instance: None,
                clone_fn: None,
                drop_fn: None,
            },
        }
    }
}

impl<T> From<&mut COptArc<T>> for Option<&mut CArc<T>> {
    fn from(copt: &mut COptArc<T>) -> Self {
        if copt.instance.is_none() {
            None
        } else {
            unsafe { (copt as *mut COptArc<T>).cast::<CArc<T>>().as_mut() }
        }
    }
}

impl<T> From<&COptArc<T>> for Option<&CArc<T>> {
    fn from(copt: &COptArc<T>) -> Self {
        if copt.instance.is_none() {
            None
        } else {
            unsafe { (copt as *const COptArc<T>).cast::<CArc<T>>().as_ref() }
        }
    }
}

impl<T> From<COptArc<T>> for Option<CArc<T>> {
    fn from(mut copt: COptArc<T>) -> Self {
        let ai = copt.instance.take();
        match copt {
            COptArc {
                instance: _,
                clone_fn: Some(clone_fn),
                drop_fn: Some(drop_fn),
            } => Some(CArc {
                instance: ai,
                clone_fn,
                drop_fn,
            }),
            _ => None,
        }
    }
}

unsafe impl<T> Opaquable for COptArc<T> {
    type OpaqueTarget = COptArc<c_void>;
}

impl<T> Default for COptArc<T> {
    fn default() -> Self {
        None.into()
    }
}

unsafe extern "C" fn c_clone<T: Sized + 'static>(
    ptr_to_arc: Option<&'static T>,
) -> Option<&'static T> {
    if let Some(p) = ptr_to_arc {
        let arc = Arc::from_raw(p);
        let cloned_arc = arc.clone();
        let _ = Arc::into_raw(arc);
        Arc::into_raw(cloned_arc).as_ref()
    } else {
        None
    }
}

unsafe extern "C" fn c_drop<T: Sized + 'static>(ptr_to_arc: Option<&T>) {
    if let Some(p) = ptr_to_arc {
        let _ = Arc::from_raw(p);
    }
}

const _: [(); std::mem::size_of::<CArc<u128>>()] = [(); std::mem::size_of::<COptArc<u128>>()];
