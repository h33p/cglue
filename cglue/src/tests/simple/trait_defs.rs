use cglue_macro::*;

#[cglue_trait]
pub trait TA {
    extern "C" fn ta_1(&self) -> usize;
}

#[cglue_trait]
pub trait TB {
    extern "C" fn tb_1(&self, val: usize) -> usize;
    fn tb_2(&self, val: usize) -> usize;
}

#[cglue_trait]
pub trait TC {
    fn tc_1(&self);
    extern "C" fn tc_2(&mut self);
}
