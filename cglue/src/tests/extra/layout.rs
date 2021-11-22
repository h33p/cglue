use super::super::simple::{structs::*, trait_defs::*, trait_groups::*};
use crate::prelude::v1::*;

cglue_trait_group!(TestGroup2, {}, { TA, TB, TC });

#[test]
fn verify_simple() {
    let obj = LayoutGuard::from(trait_obj!(SA {} as TA));
    assert!(obj.verify().is_some());
}

#[test]
fn verify_group() {
    let obj = LayoutGuard::from(group_obj!(SA {} as TestGroup));
    assert!(obj.verify().is_some());
}

#[test]
fn simple_transmute_is_invalid() {
    let obj = trait_obj!(SA {} as TA);
    let obj = unsafe { std::mem::transmute::<TABox, TBBox>(obj) };
    let obj = LayoutGuard::from(obj);
    assert!(obj.verify().is_none());
}

#[test]
fn group_transmute_is_invalid() {
    let obj = group_obj!(SA {} as TestGroup);
    let obj = unsafe { std::mem::transmute::<TestGroupBox, TestGroup2Box>(obj) };
    let obj = LayoutGuard::from(obj);
    assert!(obj.verify().is_none());
}
