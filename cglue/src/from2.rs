//! # From on Into

/// Wrapper trait that is implemented on `Into` types.
///
/// This trait is purely needed for type parameter inferring purposes, where `Into` can not be used,
/// but `From` would make it not as versatile. This trait acts like `From`, but is implemented when
/// only `Into` is implemented.
pub trait From2<T> {
    fn from2(other: T) -> Self;
}

impl<T: Into<F>, F> From2<T> for F {
    fn from2(other: T) -> Self {
        other.into()
    }
}
