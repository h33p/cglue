//! These tests are intended to check more complex casting behaviour.
use super::super::simple::structs::*;
use cglue_macro::*;

#[cglue_trait]
pub trait CustomImpl {
    #[custom_impl(
        // Types within the C interface other than self and additional wrappers.
        {
            _npo_opt: Option<&usize>,
        },
        // Unwrapped return type
        bool,
        // Conversion in trait impl to C arguments (signature names are expected).
        {
        },
        // This is the body of C impl minus the automatic wrapping.
        {
            true
        },
        // This part is processed in the trait impl after the call returns (impl_func_ret,
        // nothing extra needs to happen here).
        {
        },
    )]
    fn cimpl_1(&self, _npo_opt: Option<&usize>) -> bool {
        false
    }

    #[vtbl_only]
    #[custom_impl(
        // Types within the C interface other than self and additional wrappers.
        {
            _npo_opt: Option<&usize>,
        },
        // Unwrapped return type
        bool,
        // Conversion in trait impl to C arguments (signature names are expected).
        {
            This should not even be used in compilation!
        },
        // This is the body of C impl minus the automatic wrapping.
        {
            true
        },
        // This part is processed in the trait impl after the call returns (impl_func_ret,
        // nothing extra needs to happen here).
        {
            Neither should this!
        },
    )]
    fn cimpl_2(&self, _npo_opt: Option<&usize>) -> bool {
        false
    }

    #[custom_impl(
        // Types within the C interface other than self and additional wrappers.
        {
            _npo_opt: Option<&usize>,
        },
        // Unwrapped return type
        bool,
        // Conversion in trait impl to C arguments (signature names are expected).
        {
            let _npo_opt: usize = _npo_opt.into();
            let _npo_opt = Some(&_npo_opt);
        },
        // This is the body of C impl minus the automatic wrapping.
        {
            true
        },
        // This part is processed in the trait impl after the call returns (impl_func_ret,
        // nothing extra needs to happen here).
        {
        },
    )]
    fn cimpl_3<T: Into<usize>>(&self, _npo_opt: T) -> bool {
        false
    }
}

impl CustomImpl for SA {}

#[test]
fn test_custom1() {
    let sa = SA {};

    assert!(!sa.cimpl_1(None));

    let sa = trait_obj!(sa as CustomImpl);

    assert!(sa.cimpl_1(None));
}

#[test]
fn test_custom2() {
    let sa = SA {};

    assert!(!sa.cimpl_2(None));

    let sa = trait_obj!(sa as CustomImpl);

    assert!(!sa.cimpl_2(None));
}

#[test]
fn test_custom3() {
    let sa = SA {};

    assert!(!sa.cimpl_3(2usize));

    let sa = trait_obj!(sa as CustomImpl);

    assert!(sa.cimpl_3(4usize));
}
