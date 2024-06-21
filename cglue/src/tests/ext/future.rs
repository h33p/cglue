use cglue_macro::*;

#[test]
fn use_future() {
    async fn hii() -> u64 {
        42
    }

    let obj = trait_obj!(hii() as Future);

    impl_future(&obj);

    assert_eq!(pollster::block_on(obj), 42);
}

#[cfg(test)]
fn impl_future(_: &impl ::core::future::Future) {}
