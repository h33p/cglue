use super::super::simple::structs::*;
use super::param::*;
use cglue_macro::*;

cglue_trait_group!(GenericGroup<T> where T: Eq {}, GenericTrait<T>, { GenWithWhereClause<T>, GenWithInlineClause<T> });

cglue_impl_group!(SA, GenericGroup<T> where T: Eq {}, { GenWithWhereClause<T> });

#[test]
fn use_group_infer() {
    let sa = SA {};

    let obj = group_obj!(sa as GenericGroup<usize>);

    println!("Val: {}", obj.gt_1());
}
