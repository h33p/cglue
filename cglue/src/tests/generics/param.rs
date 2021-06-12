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

#[cglue_trait]
pub trait GenWithLifetime<'a, T: Eq + 'a> {
    fn gwl_1(&self) -> &T;
    fn gwl_2(&self) {}
}

impl<'a> GenWithLifetime<'a, usize> for SA {
    fn gwl_1(&self) -> &usize {
        &60
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

#[test]
fn use_lifetime() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenWithLifetime);

    println!("{}", obj.gwl_1());
}

#[test]
fn use_lifetime_explicit_t() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenWithLifetime<usize>);

    println!("{}", obj.gwl_1());
}

#[test]
fn use_lifetime_explicit() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenWithLifetime<'static, usize>);

    println!("{}", obj.gwl_1());
}
