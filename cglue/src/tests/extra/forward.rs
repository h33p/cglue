use crate::*;

#[allow(dead_code)]
#[cglue_forward]
trait ForwardMe {
    #[skip_func]
    fn fm_1(&self) -> &Self {
        self
    }

    #[vtbl_only]
    fn fm_2(&self) -> &Self {
        self
    }
}
