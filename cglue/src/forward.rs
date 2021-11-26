//! # Forwards a trait on references
//!
//! Combined with the `#[cglue_forward]` macro forward implementation will be generated on `Fwd`
//! type.  Whether `Fwd` implements the trait depends purely on whether the trait has
//! functions with mutable references or not.

use crate::trait_group::Opaquable;
use ::core::ops::{Deref, DerefMut};

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
pub struct Fwd<T>(pub T);

impl<T: Deref<Target = F>, F> Deref for Fwd<T> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T: DerefMut + Deref<Target = F>, F> DerefMut for Fwd<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

pub trait Forward: Sized {
    fn forward(self) -> Fwd<Self> {
        Fwd(self)
    }
}

pub trait ForwardMut: Sized {
    fn forward_mut(self) -> Fwd<Self> {
        Fwd(self)
    }
}

impl<T: Deref> Forward for T {}
impl<T: DerefMut> ForwardMut for T {}

unsafe impl<T: Opaquable> Opaquable for Fwd<T> {
    type OpaqueTarget = Fwd<T::OpaqueTarget>;
}
