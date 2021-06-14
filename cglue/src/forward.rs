//! Forwards a trait on references
//!
//! Combined with the `#[forward_trait]` macro forward implementation will be generated on `Fwd`
//! type.  Whether `Fwd` implements the trait depends purely on whether the trait has
//! functions with mutable references or not.

use core::ops::{Deref, DerefMut};

#[repr(transparent)]
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
