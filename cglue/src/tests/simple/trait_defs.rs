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
    fn tc_3(&mut self, mut _ignored: usize) {
        self.tc_2()
    }
}

#[cglue_trait]
pub trait TE {}

#[cglue_trait]
pub trait TT<T> {
    fn tt_1(&self, v: T) -> T;
}

#[cglue_trait]
pub trait TF {
    unsafe fn tf_1(&self);
    fn tf_2(self: core::pin::Pin<&Self>);
}
