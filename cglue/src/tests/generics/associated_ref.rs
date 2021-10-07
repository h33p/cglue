use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use super::super::simple::trait_groups::*;
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

#[cglue_trait]
pub trait GroupRefReturn {
    #[wrap_with_group_ref(TestGroup)]
    type ReturnType: TA + 'static;

    fn grr_1(&self) -> &Self::ReturnType;
}

impl GroupRefReturn for SA {
    type ReturnType = SA;

    fn grr_1(&self) -> &SA {
        self
    }
}

impl GroupRefReturn for SB {
    type ReturnType = SB;

    fn grr_1(&self) -> &SB {
        self
    }
}

#[cglue_trait]
pub trait GroupMutReturn {
    #[wrap_with_group_mut(TestGroup)]
    type ReturnType: TA + 'static;

    fn gmr_1(&mut self) -> &mut Self::ReturnType;
}

impl GroupMutReturn for SA {
    type ReturnType = SA;

    fn gmr_1(&mut self) -> &mut SA {
        self
    }
}

#[cglue_trait]
pub trait GroupMutReturnUnbounded {
    #[wrap_with_group_mut(TestGroup)]
    type ReturnType: TA;

    fn gmru_1(&mut self) -> &mut Self::ReturnType;
}

impl GroupMutReturnUnbounded for SA {
    type ReturnType = SA;

    fn gmru_1(&mut self) -> &mut SA {
        self
    }
}

#[cglue_trait]
pub trait GroupLtMutReturn<'a> {
    #[wrap_with_group_mut(TestGroup)]
    type ReturnType: TA + 'a;

    fn glmr_1(&'a mut self) -> &'a mut Self::ReturnType;
}

impl<'a> GroupLtMutReturn<'a> for SA {
    type ReturnType = SA;

    fn glmr_1(&mut self) -> &mut SA {
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

#[test]
fn use_group_ref() {
    let sa = SB {};

    let obj = trait_obj!(sa as GroupRefReturn);

    let obj2 = obj.grr_1();

    assert_eq!(obj2.ta_1(), 6);
}

#[test]
fn use_group_mut() {
    let sa = SA {};

    let mut obj = trait_obj!(sa as GroupMutReturn);

    let obj2 = obj.gmr_1();

    assert_eq!(obj2.ta_1(), 5);
}

#[test]
fn use_group_mut_unbounded() {
    let mut sa = SA {};

    let mut obj = trait_obj!(&mut sa as GroupMutReturnUnbounded);

    let obj2 = obj.gmru_1();

    assert_eq!(obj2.ta_1(), 5);
}

#[test]
fn use_group_lt_mut() {
    let sa = SA {};

    let mut obj = trait_obj!(sa as GroupLtMutReturn);

    let obj2 = obj.glmr_1();

    assert_eq!(obj2.ta_1(), 5);
}
