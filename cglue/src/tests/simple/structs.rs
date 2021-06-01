use super::trait_defs::*;
use cglue_macro::*;

pub struct SA {}
pub struct SB {}

impl TA for SA {
    extern "C" fn ta_1(&self) -> usize {
        5
    }
}

impl TA for SB {
    extern "C" fn ta_1(&self) -> usize {
        6
    }
}

impl TB for SB {
    extern "C" fn tb_1(&self, val: usize) -> usize {
        val * 2
    }

    fn tb_2(&self, val: usize) -> usize {
        val * val
    }
}

cglue_impl_group!(SB, super::trait_groups::TestGroup, { TB });

impl TC for SA {
    fn tc_1(&self) {}
    extern "C" fn tc_2(&mut self) {}
}

#[test]
fn call_a() {
    let a = SA {};
    let mut b = SB {};
    let c = SB {};

    let obja = trait_obj!(&a as TA);
    let objb = trait_obj!(&mut b as TA);
    let objc = trait_obj!(c as TA);

    assert_eq!(obja.ta_1() + objb.ta_1() + objc.ta_1(), 17);
}

#[test]
fn get_b() {
    let b = SB {};

    let objb = trait_obj!(b as TB);

    assert_eq!(objb.tb_2(objb.tb_1(10)), 400);
}
