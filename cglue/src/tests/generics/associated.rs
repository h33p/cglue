use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use cglue_macro::*;
use core::ffi::c_void;

#[cglue_trait]
pub trait AssociatedReturn {
    #[wrap_with(*const c_void)]
    #[return_wrap(|ret| Box::leak(Box::new(ret)) as *mut _ as *const c_void)]
    type ReturnType;

    fn ar_1(&self) -> Self::ReturnType;
}

impl AssociatedReturn for SA {
    type ReturnType = usize;

    fn ar_1(&self) -> usize {
        42
    }
}

#[cglue_trait]
pub trait ObjReturn {
    #[wrap_with_obj(TA)]
    type ReturnType: TA + 'static;

    fn or_1(&self) -> Self::ReturnType;
}

impl ObjReturn for SA {
    type ReturnType = SA;

    fn or_1(&self) -> SA {
        SA {}
    }
}

#[cglue_trait]
pub trait ObjUnboundedReturn {
    #[wrap_with_obj(TA)]
    type ReturnType: TA;

    fn our_1(&self) -> Self::ReturnType;
}

impl ObjUnboundedReturn for SA {
    type ReturnType = SB;

    fn our_1(&self) -> SB {
        SB {}
    }
}

#[test]
fn use_assoc_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as AssociatedReturn);

    let ret = obj.ar_1();

    println!("{:?}", ret);

    assert_eq!(unsafe { *(ret as *const usize) }, 42);
}

#[test]
fn use_obj_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as ObjReturn);

    let ta = obj.or_1();

    assert_eq!(ta.ta_1(), 5);
}
