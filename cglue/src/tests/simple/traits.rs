//! These tests are intended to check more complex casting behaviour.
use cglue_macro::*;

#[cglue_trait]
pub trait WithSlice {
    fn wslice_1(&mut self, _slc: &[usize]) {}
}

#[cglue_trait]
pub trait WithOptions {
    fn wopt_1(&self, _npo_opt: Option<&usize>) {}
    fn wopt_2(&self, _opt: Option<usize>) {}
    fn wopt_3(&mut self, _npo_option: Option<&u128>, _wrap_option: Option<u128>) {}
}

#[cglue_trait]
#[int_result]
pub trait WithIntResult {
    fn wint_1(&self, val: usize) -> Result<usize, std::io::Error> {
        Ok(val)
    }
    #[no_int_result]
    fn wint_2(&self, val: usize) -> Result<usize, usize> {
        Ok(val)
    }
}

type AliasResult<T, E> = Result<T, E>;

#[cglue_trait]
#[int_result(AliasResult)]
pub trait WithAliasIntResult {
    fn waint_1(&self, val: usize) -> AliasResult<usize, std::io::Error> {
        Ok(val)
    }
    #[no_int_result]
    fn waint_2(&self, val: usize) -> AliasResult<usize, usize> {
        Ok(val)
    }
}

struct Implementor {}

impl WithSlice for Implementor {}
impl WithOptions for Implementor {}
impl WithIntResult for Implementor {}
impl WithAliasIntResult for Implementor {}

#[test]
fn slices_wrapped() {
    let vtbl = <&WithSliceVtbl<&mut Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&mut Implementor, *const usize, usize) = vtbl.wslice_1();
}

#[test]
fn npo_option_forwarded() {
    let vtbl = <&WithOptionsVtbl<&mut Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, Option<&usize>) = vtbl.wopt_1();
}

#[test]
fn non_npo_option_wrapped() {
    let vtbl = <&WithOptionsVtbl<&mut Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, crate::option::COption<usize>) = vtbl.wopt_2();
}

#[test]
fn mixed_options() {
    let vtbl = <&WithOptionsVtbl<&mut Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&mut Implementor, Option<&u128>, crate::option::COption<u128>) =
        vtbl.wopt_3();
}

#[test]
fn int_result() {
    let vtbl = <&WithIntResultVtbl<&Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, usize, &mut core::mem::MaybeUninit<usize>) -> i32 =
        vtbl.wint_1();
}

#[test]
fn no_int_result() {
    let vtbl = <&WithIntResultVtbl<&Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, usize) -> crate::result::CResult<usize, usize> =
        vtbl.wint_2();
}

#[test]
fn alias_int_result() {
    let vtbl = <&WithAliasIntResultVtbl<&Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, usize, &mut core::mem::MaybeUninit<usize>) -> i32 =
        vtbl.waint_1();
}

#[test]
fn alias_no_int_result() {
    let vtbl = <&WithAliasIntResultVtbl<&Implementor, Implementor, (), ()>>::default();
    let _: unsafe extern "C" fn(&Implementor, usize) -> crate::result::CResult<usize, usize> =
        vtbl.waint_2();
}
