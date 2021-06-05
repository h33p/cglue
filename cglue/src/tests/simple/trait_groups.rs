//! These tests check definition and usage of different trait groups
use super::structs::*;
use super::trait_defs::*;
use cglue_macro::*;

cglue_trait_group!(TestGroup, TA, { TB, TC });
cglue_impl_group!(SA, TestGroup, { TC });
cglue_impl_group!(SB, super::trait_groups::TestGroup, { TB });

#[test]
fn test_group() {
    let a = SA {};

    let _ = group_obj!(&a as TestGroup);

    let group = group_obj!(a as TestGroup);

    {
        let group = as_ref!(group impl TC).unwrap();
        group.tc_1();
    }

    assert!(!check!(group impl TB));

    let cast = cast!(group impl TC).unwrap();

    let mut group = TestGroup::from(cast);

    assert!(as_mut!(group impl TB).is_none());
}

#[test]
fn test_group_2() {
    let mut b = SB {};

    let group = group_obj!(&mut b as TestGroup);
    assert!(check!(group impl TB));

    let group = group_obj!(&b as TestGroup);
    assert!(check!(group impl TB));

    let group = group_obj!(b as TestGroup);
    assert!(check!(group impl TB));
}
