use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use cglue_macro::*;

cglue_trait_group!(MaybeDebug, { TA }, { ::ext::core::fmt::Debug });
cglue_impl_group!(SA, MaybeDebug, Debug);

cglue_trait_group!(NumberFormat, { Debug, Display,
    ::ext::core::fmt::Octal,
    ::ext::core::fmt::LowerHex,
    ::ext::core::fmt::UpperHex,
    ::ext::core::fmt::Binary,
}, {});
cglue_impl_group!(usize, NumberFormat);

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
    let _ = format!("{:?}", t);
}

#[test]
fn use_display() {
    let v = 42;

    let obj = trait_obj!(v as Display);

    assert_eq!("42", &format!("{}", obj));
}

#[test]
fn use_num_fmt() {
    let v = 42;

    let obj = group_obj!(v as NumberFormat);

    assert_eq!(&format!("{}", v), &format!("{}", obj));
    assert_eq!(&format!("{:?}", v), &format!("{:?}", obj));
    assert_eq!(&format!("{:x}", v), &format!("{:x}", obj));
    assert_eq!(&format!("{:X}", v), &format!("{:X}", obj));
    assert_eq!(&format!("{:b}", v), &format!("{:b}", obj));
}
