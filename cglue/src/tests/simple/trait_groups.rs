//! These tests check definition and usage of different trait groups
use super::structs::*;
use super::trait_defs::*;
use cglue_macro::*;

cglue_trait_group!(TestGroup, TA, { TB, TC });
cglue_impl_group!(SA, TestGroup, { TC });

cglue_impl_group!(&'a SA, TestGroup, {});
cglue_impl_group!(SB, super::trait_groups::TestGroup, { TB });

cglue_trait_group!(TestGroupGen, TT<u8>, { TT<usize> = TTUsize, TT<u64> = TTUSixtyFour });
cglue_impl_group!(SA, TestGroupGen, { TT<usize> = TTUsize });

#[test]
fn test_group() {
    let mut a = SA {};

    // Slight regression in 0.2, can not use TestGroupRef, because
    // (optional) TC requires mutable refs for the impl to work.
    // Can be fixed through unstable features or forcing boxing
    // and a ref group impl.
    //
    // Having both requres explicitly specifying inner type like done here.
    fn into_test<'a, T: Into<TestGroupBaseBox<'a, T>> + Send + 'a>(t: T) -> TestGroupBox<'a> {
        group_obj!(t as TestGroup)
    }
    let _ = into_test(&a);

    let _ = group_obj!(&mut a as TestGroup);

    let group = group_obj!(a as TestGroup);

    {
        let group = as_ref!(group impl TC).unwrap();
        group.tc_1();
    }

    assert!(!check!(group impl TB));

    let cast = cast!(group impl TC).unwrap();

    let mut group = cast.upcast();

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

    let tup = (&group, ());
    assert!(check!(tup.0 impl TB));

    let tb = as_ref!(group impl TB).unwrap();

    tb.tb_1(1);
}

#[test]
fn test_group_3() {
    let mut a = SA {};

    let group = group_obj!(&mut a as TestGroupGen);
    assert!(check!(group impl TTUsize));
    #[cfg(not(feature = "unstable"))]
    assert!(!check!(group impl TTUSixtyFour));
    #[cfg(feature = "unstable")]
    assert!(check!(group impl TTUSixtyFour));

    let tusize = as_ref!(group impl TTUsize).unwrap();

    tusize.tt_1(1usize);
}
