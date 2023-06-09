//! These tests are intended to check more complex casting behaviour.
use crate::slice::*;
use cglue_macro::*;

#[cglue_trait]
pub trait WithSlice {
    fn wslice_1(&mut self, _slc: &[usize]) {}
    fn wslice_2(&mut self, _str: &str) {}
    fn wslice_3(&mut self) -> &str {
        "slice"
    }
    fn wslice_4<'a>(&'a mut self, _val: &str) -> &'a str {
        "slice"
    }
}

#[cglue_trait]
pub trait WithOptions {
    fn wopt_1(&self, _npo_opt: Option<&usize>) {}
    fn wopt_2(&self, _opt: Option<usize>) {}
    fn wopt_3(&mut self, _npo_option: Option<&u64>, _wrap_option: Option<u64>) {}
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

#[cglue_trait]
pub trait WithInto {
    fn winto_1(&self, _into: impl Into<usize>) {}
}

struct Implementor {}

impl WithSlice for Implementor {}
impl WithOptions for Implementor {}
impl WithIntResult for Implementor {}
impl WithAliasIntResult for Implementor {}
impl WithInto for Implementor {}

type ICont<I, C> = crate::trait_group::CGlueObjContainer<I, crate::trait_group::NoContext, C>;
type IRefCont<C> = ICont<&'static Implementor, C>;
type IMutCont<C> = ICont<&'static mut Implementor, C>;

type WSCont = IMutCont<WithSliceRetTmp<crate::trait_group::NoContext>>;
type WOCont = IMutCont<WithOptionsRetTmp<crate::trait_group::NoContext>>;
type WIRCont = IRefCont<WithIntResultRetTmp<crate::trait_group::NoContext>>;
type WAIRCont = IRefCont<WithAliasIntResultRetTmp<crate::trait_group::NoContext>>;
type WINTOCont = IRefCont<WithIntoRetTmp<crate::trait_group::NoContext>>;

#[test]
fn slices_wrapped() {
    let vtbl = <&WithSliceVtbl<WSCont>>::default();
    let _: unsafe extern "C" fn(&mut WSCont, CSliceRef<usize>) = vtbl.wslice_1();
    let _: unsafe extern "C" fn(&mut WSCont, CSliceRef<u8>) = vtbl.wslice_2();
    let _: unsafe extern "C" fn(&mut WSCont) -> CSliceRef<u8> = vtbl.wslice_3();
    let _: for<'a> unsafe extern "C" fn(&'a mut WSCont, CSliceRef<u8>) -> CSliceRef<'a, u8> =
        vtbl.wslice_4();
}

#[test]
fn npo_option_forwarded() {
    let vtbl = <&WithOptionsVtbl<WOCont>>::default();
    let _: unsafe extern "C" fn(&WOCont, Option<&usize>) = vtbl.wopt_1();
}

#[test]
fn non_npo_option_wrapped() {
    let vtbl = <&WithOptionsVtbl<WOCont>>::default();
    let _: unsafe extern "C" fn(&WOCont, crate::option::COption<usize>) = vtbl.wopt_2();
}

#[test]
fn mixed_options() {
    let vtbl = <&WithOptionsVtbl<WOCont>>::default();
    let _: unsafe extern "C" fn(&mut WOCont, Option<&u64>, crate::option::COption<u64>) =
        vtbl.wopt_3();
}

#[test]
fn int_result() {
    let vtbl = <&WithIntResultVtbl<WIRCont>>::default();
    let _: unsafe extern "C" fn(&WIRCont, usize, &mut core::mem::MaybeUninit<usize>) -> i32 =
        vtbl.wint_1();
}

#[test]
fn no_int_result() {
    let vtbl = <&WithIntResultVtbl<WIRCont>>::default();
    let _: unsafe extern "C" fn(&WIRCont, usize) -> crate::result::CResult<usize, usize> =
        vtbl.wint_2();
}

#[test]
fn alias_int_result() {
    let vtbl = <&WithAliasIntResultVtbl<WAIRCont>>::default();
    let _: unsafe extern "C" fn(&WAIRCont, usize, &mut core::mem::MaybeUninit<usize>) -> i32 =
        vtbl.waint_1();
}

#[test]
fn alias_no_int_result() {
    let vtbl = <&WithAliasIntResultVtbl<WAIRCont>>::default();
    let _: unsafe extern "C" fn(&WAIRCont, usize) -> crate::result::CResult<usize, usize> =
        vtbl.waint_2();
}

#[test]
fn into_t_wrapped() {
    let vtbl = <&WithIntoVtbl<WINTOCont>>::default();
    let _: unsafe extern "C" fn(&WINTOCont, usize) = vtbl.winto_1();
}
