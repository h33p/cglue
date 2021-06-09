use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use cglue_macro::*;

cglue_trait_group!(MaybeDebug, { TA }, { ::ext::core::fmt::Debug });
cglue_impl_group!(SA, MaybeDebug, Debug);

#[test]
fn use_debug() {
    let sa = SA {};
    let obj = trait_obj!(sa as Debug);
    impl_debug(&obj);

    println!("{:?}", obj);

    assert_eq!("SA", &format!("{:?}", obj));
}

#[test]
fn use_debug_group() {
    let sa = SA {};
    let obj = group_obj!(sa as MaybeDebug);
    let obj = as_ref!(obj impl Debug).unwrap();
    impl_debug(obj)
}

#[cfg(test)]
fn impl_debug(t: &impl ::core::fmt::Debug) {
    let _ = t.clone();
}
