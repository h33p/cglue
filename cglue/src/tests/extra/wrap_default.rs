use super::super::simple::structs::*;
use cglue_macro::*;

pub struct ExtraFeatureWrap<T> {
    _v: T,
}

#[cglue_trait]
pub trait ExtraFeature {
    fn ef_1(&self) -> usize;
}

#[cglue_trait]
pub trait Basic: Sized + Send + Sync {
    #[vtbl_only('_, wrap_with_obj(ExtraFeature))]
    fn b_1(&self) -> ExtraFeatureWrap<&Self> {
        ExtraFeatureWrap { _v: self }
    }

    #[vtbl_only('static, wrap_with_obj(ExtraFeature))]
    fn b_2(self) -> ExtraFeatureWrap<Self> {
        ExtraFeatureWrap { _v: self }
    }

    #[vtbl_only('_, wrap_with_obj(ExtraFeature))]
    fn b_3(&self, _arg: bool) -> ExtraFeatureWrap<&Self> {
        ExtraFeatureWrap { _v: self }
    }

    #[vtbl_only('static, wrap_with_obj(ExtraFeature))]
    fn b_4(self, _arg: bool, _a2: u8) -> ExtraFeatureWrap<Self> {
        ExtraFeatureWrap { _v: self }
    }
}

impl<T> ExtraFeature for ExtraFeatureWrap<T> {
    fn ef_1(&self) -> usize {
        42
    }
}

impl ExtraFeature for SA {
    fn ef_1(&self) -> usize {
        43
    }
}

impl Basic for SA {}

#[test]
fn test_wrap() {
    let basic = trait_obj!(SA {} as Basic);
    assert_eq!(basic.b_1().ef_1(), 42);
}
