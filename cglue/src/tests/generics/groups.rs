use super::super::simple::structs::*;
use super::param::*;
use cglue_macro::*;

cglue_trait_group!(GenericGroup<T> where T: Eq {}, GenericTrait<T>, { GenWithWhereClause<T>, GenWithInlineClause<T> });

cglue_trait_group!(MixedGenericGroup<T, F> where F: Eq, T: Eq {}, GenericTrait<F>, { GenWithWhereClause<F>, GenWithInlineClause<T> });

cglue_impl_group!(SA, GenericGroup<T> where T: Eq {}, { GenWithInlineClause<T> });

cglue_impl_group!(SA, MixedGenericGroup<T, F> where F: Eq, T: Eq {}, { GenWithWhereClause<F> });

#[test]
fn use_group_infer() {
    let sa = SA {};

    let obj = group_obj!(sa as GenericGroup);

    println!("Val: {}", obj.gt_1());
}

#[test]
fn use_group_explicit() {
    let sa = SA {};

    let obj = group_obj!(sa as GenericGroup<usize>);

    println!("Val: {}", obj.gt_1());
}

#[test]
fn use_mixed_group_partial_infer() {
    let sa = SA {};

    let obj = group_obj!(sa as MixedGenericGroup<usize, _>);

    println!("Val: {}", obj.gt_1());
}

#[test]
fn use_mixed_group_explicit() {
    let sa = SA {};

    let obj = group_obj!(sa as MixedGenericGroup<usize, usize>);

    println!("Val: {}", obj.gt_1());
}
