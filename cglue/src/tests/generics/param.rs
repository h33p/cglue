use super::super::simple::structs::*;
use cglue_macro::*;

//#[cglue_trait]
pub trait GenericTrait<T> {
    fn gt_1(&self) -> T;
}

impl GenericTrait<usize> for SA {
    fn gt_1(&self) -> usize {
        27
    }
}
