//! # FFI-safe Arc.
use crate::trait_group::c_void;
use crate::trait_group::Opaquable;
use std::sync::Arc;

unsafe impl<T: Sync + Send> Send for CArc<T> {}
unsafe impl<T: Sync + Send> Sync for CArc<T> {}

/// FFI-Safe Arc
///
/// This is an FFI-Safe equivalent of Arc<T> and Option<Arc<T>>.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct CArc<T: Sized + 'static> {
    instance: Option<&'static T>,
    clone_fn: Option<unsafe extern "C" fn(Option<&'static T>) -> Option<&'static T>>,
    drop_fn: Option<unsafe extern "C" fn(Option<&T>)>,
}

impl<T: Sized + 'static> AsRef<Option<&'static T>> for CArc<T> {
    fn as_ref(&self) -> &Option<&'static T> {
        &self.instance
    }
}

impl<T> Clone for CArc<T> {
    fn clone(&self) -> Self {
        match <Option<&CArcSome<T>>>::from(self) {
            Some(arc) => Some(arc.clone()).into(),
            None => Default::default(),
        }
    }
}

impl<T> Drop for CArc<T> {
    fn drop(&mut self) {
        if let Some(arc) = <Option<&mut CArcSome<T>>>::from(self) {
            unsafe { std::ptr::drop_in_place(arc) };
        }
    }
}

impl<T> CArc<T> {
    /// Take arc's resources, and leave `None` in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// use cglue::arc::CArc;
    ///
    /// let mut arc = CArc::from(0u64);
    ///
    /// assert!(arc.as_ref().is_some());
    ///
    /// let arc2 = arc.take();
    ///
    /// assert!(arc2.as_ref().is_some());
    /// assert!(arc.as_ref().is_none());
    /// ```
    pub fn take(&mut self) -> CArc<T> {
        Self {
            instance: self.instance.take(),
            clone_fn: self.clone_fn.take(),
            drop_fn: self.drop_fn.take(),
        }
    }
}

impl<T> From<Option<CArcSome<T>>> for CArc<T> {
    fn from(opt: Option<CArcSome<T>>) -> Self {
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

impl<T> From<T> for CArc<T> {
    fn from(obj: T) -> Self {
        Self::from(Arc::new(obj))
    }
}

impl<T> From<Arc<T>> for CArc<T> {
    fn from(arc: Arc<T>) -> Self {
        Self {
            instance: unsafe { Arc::into_raw(arc).as_ref() },
            clone_fn: Some(c_clone),
            drop_fn: Some(c_drop),
        }
    }
}

impl<T> From<Option<Arc<T>>> for CArc<T> {
    fn from(arc: Option<Arc<T>>) -> Self {
        match arc {
            Some(arc) => Self::from(arc),
            _ => Default::default(),
        }
    }
}

impl<T> From<&mut CArc<T>> for Option<&mut CArcSome<T>> {
    fn from(copt: &mut CArc<T>) -> Self {
        if copt.instance.is_none() {
            None
        } else {
            unsafe { (copt as *mut CArc<T>).cast::<CArcSome<T>>().as_mut() }
        }
    }
}

impl<T> From<&CArc<T>> for Option<&CArcSome<T>> {
    fn from(copt: &CArc<T>) -> Self {
        if copt.instance.is_none() {
            None
        } else {
            unsafe { (copt as *const CArc<T>).cast::<CArcSome<T>>().as_ref() }
        }
    }
}

impl<T> From<CArc<T>> for Option<CArcSome<T>> {
    fn from(mut copt: CArc<T>) -> Self {
        let ai = copt.instance.take();
        match copt {
            CArc {
                instance: _,
                clone_fn: Some(clone_fn),
                drop_fn: Some(drop_fn),
            } => Some(CArcSome {
                instance: ai,
                clone_fn,
                drop_fn,
            }),
            _ => None,
        }
    }
}

unsafe impl<T> Opaquable for CArc<T> {
    type OpaqueTarget = CArc<c_void>;
}

impl<T> Default for CArc<T> {
    fn default() -> Self {
        Self {
            instance: None,
            clone_fn: None,
            drop_fn: None,
        }
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

const _: [(); std::mem::size_of::<CArcSome<u128>>()] = [(); std::mem::size_of::<CArc<u128>>()];

/// FFI-Safe Arc
///
/// This Arc essentially uses clone/drop from the module that created it, to not mix up global
/// allocators.
#[repr(C)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
struct CArcSome<T: Sized + 'static> {
    instance: Option<&'static T>,
    clone_fn: unsafe extern "C" fn(Option<&'static T>) -> Option<&'static T>,
    drop_fn: unsafe extern "C" fn(Option<&T>),
}

unsafe impl<T: Sync + Send> Send for CArcSome<T> {}
unsafe impl<T: Sync + Send> Sync for CArcSome<T> {}

impl<T> Clone for CArcSome<T> {
    fn clone(&self) -> Self {
        Self {
            instance: unsafe { (self.clone_fn)(self.instance) },
            ..*self
        }
    }
}

impl<T> Drop for CArcSome<T> {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.instance) }
    }
}

impl<T> AsRef<T> for CArcSome<T> {
    fn as_ref(&self) -> &T {
        self.instance.unwrap()
    }
}

unsafe impl<T> Opaquable for CArcSome<T> {
    type OpaqueTarget = CArcSome<c_void>;
}
