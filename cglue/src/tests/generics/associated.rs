use super::super::simple::structs::*;
use super::super::simple::trait_defs::*;
use super::groups::*;
use super::param::*;
use crate::trait_group::c_void;
use cglue_macro::*;

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
#[int_result]
pub trait ObjResultReturn {
    #[wrap_with_obj(TA)]
    type ReturnType: TA + 'static;

    #[allow(clippy::result_unit_err)]
    fn orr_1(&self) -> Result<Self::ReturnType, ()>;

    #[no_int_result]
    #[allow(clippy::result_unit_err)]
    fn orr_2(&self) -> Result<Self::ReturnType, ()> {
        self.orr_1()
    }
}

impl ObjResultReturn for SA {
    type ReturnType = SA;

    fn orr_1(&self) -> Result<SA, ()> {
        Ok(SA {})
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

#[cglue_trait]
pub trait GenericReturn<T: 'static> {
    #[wrap_with_obj(GenericTrait<T>)]
    type ReturnType: GenericTrait<T>;

    fn gr_1(&self) -> Self::ReturnType;
}

impl GenericReturn<usize> for SA {
    type ReturnType = SA;

    fn gr_1(&self) -> SA {
        SA {}
    }
}

// TODO: generic return where T gets automatically bounded by cglue_trait

#[cglue_trait]
pub trait GenericGroupReturn<T: 'static + Eq> {
    #[wrap_with_group(GenericGroup<T>)]
    type ReturnType: GenericTrait<T>;

    fn ggr_1(&self) -> Self::ReturnType;
}

impl GenericGroupReturn<usize> for SA {
    type ReturnType = SA;

    fn ggr_1(&self) -> SA {
        SA {}
    }
}

#[cglue_trait]
pub trait GenericConsumedGroupReturn<T: 'static + Eq> {
    #[wrap_with_group(GenericGroup<T>)]
    type ReturnType: GenericTrait<T>;

    fn gcgr_1(self) -> Self::ReturnType;
}

impl GenericConsumedGroupReturn<usize> for SA {
    type ReturnType = SA;

    fn gcgr_1(self) -> SA {
        self
    }
}

#[cglue_trait]
pub trait UnwrappedAssociatedVar {
    type AssocVar;
}

#[cglue_trait]
pub trait UnwrappedAssociatedReturn {
    type ReturnType;

    fn uar_1(self) -> Self::ReturnType;
}

impl UnwrappedAssociatedReturn for SA {
    type ReturnType = SA;

    fn uar_1(self) -> SA {
        self
    }
}

cglue_trait_group!(
    UnwrappedGroup<T>,
    UnwrappedAssociatedReturn<ReturnType = T>,
    {}
);
cglue_impl_group!(SA, UnwrappedGroup<T = SA>);

#[test]
fn use_assoc_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as AssociatedReturn);

    let ret = obj.ar_1();

    println!("{:?}", ret);

    // SAFETY: the underlying type is a usize box, we are just testing.
    let b = unsafe { Box::from_raw(ret as *mut usize) };
    assert_eq!(*b, 42);
}

#[test]
fn use_obj_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as ObjReturn);

    let ta = obj.or_1();

    assert_eq!(ta.ta_1(), 5);
}

#[test]
fn use_gen_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenericReturn);

    let ta = obj.gr_1();

    assert_eq!(ta.gt_1(), 27);
}

#[test]
fn use_group_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenericGroupReturn);

    let group = obj.ggr_1();

    let cast = cast!(group impl GenWithInlineClause).unwrap();

    assert!(cast.gwi_1(&cast.gt_1()));
    assert!(!cast.gwi_1(&(cast.gt_1() + 1)));
}

#[test]
fn use_consumed_group_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenericConsumedGroupReturn);

    let group = obj.gcgr_1();

    let cast = cast!(group impl GenWithInlineClause).unwrap();

    assert!(cast.gwi_1(&cast.gt_1()));
    assert!(!cast.gwi_1(&(cast.gt_1() + 1)));
}

#[test]
fn use_unwrapped_associated_return() {
    let sa = SA {};

    let obj = trait_obj!(sa as UnwrappedAssociatedReturn);

    let _sa: SA = obj.uar_1();
}

#[test]
fn use_unwrapped_group() {
    let sa = SA {};

    let obj = group_obj!(sa as UnwrappedGroup);

    let _sa: SA = obj.uar_1();
}
