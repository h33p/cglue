use super::structs::*;
use cglue_macro::*;

#[cglue_trait]
pub trait TOnlyConsuming {
    fn toc_1(self) -> usize;
}

impl TOnlyConsuming for SA {
    fn toc_1(self) -> usize {
        57
    }
}

#[cglue_trait]
pub trait TMixedConsuming {
    fn tmc_1(self) -> usize;
    fn tmc_2(&self) -> usize;
}

impl TMixedConsuming for SA {
    fn tmc_1(self) -> usize {
        43
    }

    fn tmc_2(&self) -> usize {
        42
    }
}

cglue_trait_group!(ConsumerGroup, TOnlyConsuming, TMixedConsuming);

cglue_impl_group!(SA, ConsumerGroup, TMixedConsuming);

#[test]
fn use_consuming() {
    let sa = SA {};

    let obj = trait_obj!(sa as TOnlyConsuming);

    assert_eq!(obj.toc_1(), 57);
}

#[test]
fn use_mixed_consuming() {
    let sa = SA {};

    let obj = trait_obj!(sa as TMixedConsuming);

    assert_eq!(obj.tmc_2(), 42);
    assert_eq!(obj.tmc_1(), 43);
}

#[test]
fn use_group_consuming() {
    let sa = SA {};

    let obj = group_obj!(sa as ConsumerGroup);

    let obj = cast!(obj impl TMixedConsuming).unwrap();

    assert_eq!(obj.tmc_2(), 42);
    assert_eq!(obj.tmc_1(), 43);
}
