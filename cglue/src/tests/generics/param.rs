use super::super::simple::structs::*;
use cglue_macro::*;

#[cglue_trait]
pub trait GenericTrait<T> {
    fn gt_1(&self) -> T;
}

impl GenericTrait<usize> for SA {
    fn gt_1(&self) -> usize {
        27
    }
}

#[cglue_trait]
pub trait GenWithWhereClause<T>
where
    T: Eq,
{
    fn gww_1(&self, input: &T) -> bool;
}

impl GenWithWhereClause<usize> for SA {
    fn gww_1(&self, input: &usize) -> bool {
        self.gt_1().eq(input)
    }
}

#[cglue_trait]
pub trait GenWithInlineClause<T: Eq> {
    fn gwi_1(&self, input: &T) -> bool;
}

impl GenWithInlineClause<usize> for SA {
    fn gwi_1(&self, input: &usize) -> bool {
        self.gt_1().eq(input)
    }
}

#[test]
fn use_gen_infer() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenericTrait);

    println!("{}", obj.gt_1());
}

#[test]
fn use_gen_explicit() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenericTrait<usize>);

    println!("{}", obj.gt_1());
}

#[no_mangle]
pub extern "C" fn get_gen() -> CGlueOpaqueTraitObjGenericTrait<'static, usize> {
    trait_obj!(SA {} as GenericTrait<usize>)
}

use crate::tests::simple::trait_groups::*;

#[no_mangle]
pub extern "C" fn use_groups(a: TestGroupOpaqueMut, b: TestGroupOpaqueBox, c: TestGroupOpaqueRef) {}
