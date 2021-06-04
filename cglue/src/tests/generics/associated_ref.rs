use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use cglue_macro::*;

#[cglue_trait]
pub trait ObjRefReturn {
    #[wrap_with_obj_ref(TA)]
    type ReturnType: TA + 'static;

    fn orr_1(&self) -> &Self::ReturnType;
}

impl ObjRefReturn for SA {
    type ReturnType = SA;

    fn orr_1(&self) -> &SA {
        self
    }
}

#[cglue_trait]
pub trait ObjMutReturn {
    #[wrap_with_obj_mut(TA)]
    type ReturnType: TA + 'static;

    fn omr_1(&mut self) -> &mut Self::ReturnType;
}

impl ObjMutReturn for SA {
    type ReturnType = SA;

    fn omr_1(&mut self) -> &mut SA {
        self
    }
}

#[test]
fn use_assoc_ref() {
    let sa = SA {};

    let obj = trait_obj!(sa as ObjRefReturn);

    let obj2 = obj.orr_1();

    assert_eq!(obj2.ta_1(), 5);
}

#[test]
fn use_assoc_mut() {
    let sa = SA {};

    let mut obj = trait_obj!(sa as ObjMutReturn);

    let obj2 = obj.omr_1();

    assert_eq!(obj2.ta_1(), 5);
}
