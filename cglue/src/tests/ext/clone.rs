use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use cglue_macro::*;

cglue_trait_group!(MaybeClone, { TA }, { Clone });
cglue_impl_group!(SA, MaybeClone, Clone);

#[test]
fn use_clone() {
    let sa = SA {};
    let obj = trait_obj!(sa as Clone);
    impl_clone(&obj)
}

#[test]
fn use_clone_group() {
    let sa = SA {};
    let obj = group_obj!(sa as MaybeClone);
    let obj = as_ref!(obj impl Clone).unwrap();
    impl_clone(obj)
}

#[cfg(test)]
fn impl_clone(t: &impl ::core::clone::Clone) {
    let _ = t.clone();
}
