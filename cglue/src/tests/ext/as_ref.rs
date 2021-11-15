use super::super::simple::structs::*;
use cglue_macro::*;

cglue_trait_group!(MaybeAsRef<T>, {}, ::ext::core::convert::AsRef<T>, {}, false);
cglue_impl_group!(SA, MaybeAsRef<SA>, ::ext::core::convert::AsRef<SA>);

#[test]
fn use_as_ref() {
    let sa = SA {};
    let obj = trait_obj!(sa as ::ext::core::convert::AsRef);
    impl_as_ref(&obj)
}

#[test]
fn use_as_ref_group() {
    let sa = SA {};
    let obj = group_obj!(sa as MaybeAsRef<SA>);
    let obj = as_ref!(obj impl AsRef).unwrap();
    impl_as_ref(obj)
}

#[cfg(test)]
fn impl_as_ref<T>(t: &impl ::core::convert::AsRef<T>) {
    let _ = t.as_ref();
}
