use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use super::super::simple::trait_groups::*;
use cglue_macro::*;

#[cglue_trait]
pub trait GroupGatReturn {
    #[wrap_with_group(TestGroup)]
    type ReturnType<'abc>: TA + 'abc
    where
        Self: 'abc;

    fn ggr_1<'a>(&'a mut self, val: &'a u32) -> Self::ReturnType<'a>;
}

impl GroupGatReturn for SA {
    type ReturnType<'a> = &'a SA;

    fn ggr_1(&mut self, _val: &u32) -> &SA {
        self
    }
}

#[test]
fn use_gat_return() {
    use crate::prelude::v1::*;
    let sa = SA {};
    let mut obj = trait_obj!(sa as GroupGatReturn);
    let ta = obj.ggr_1(&0);
    assert_eq!(ta.ta_1(), 5);
}
