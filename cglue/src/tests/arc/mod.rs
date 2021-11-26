use super::simple::structs::*;
use crate::arc::*;
//use crate::boxed::*;
use crate::*;
use std::sync::Arc;

#[cglue_trait]
pub trait DoThings {
    fn dt_1(&self) -> usize;
}

impl DoThings for SA {
    fn dt_1(&self) -> usize {
        55
    }
}

#[cglue_trait]
pub trait DoThingsSend: Send {
    fn dts_1(&self) -> usize;
}

impl DoThingsSend for SA {
    fn dts_1(&self) -> usize {
        55
    }
}

#[cglue_trait]
pub trait DoerGetter {
    #[wrap_with_obj(DoThings)]
    type ReturnType: DoThings;

    fn dget_1(&self) -> Self::ReturnType;
}

impl DoerGetter for SA {
    type ReturnType = SA;

    fn dget_1(&self) -> Self::ReturnType {
        SA {}
    }
}

/*#[test]
fn use_dothings() {
    let sa = SA {};
    let wrapped = CBox::from((sa, CArc::from(()).into_opt()));
    assert_eq!(wrapped.dt_1(), 55);
}*/

#[test]
fn use_getter_obj() {
    let sa = SA {};

    let arc = std::sync::Arc::from(());

    assert_eq!(Arc::strong_count(&arc), 1);

    let opt_arc = CArc::<()>::from(arc.clone());

    assert_eq!(Arc::strong_count(&arc), 2);

    let getter = trait_obj!((sa, opt_arc) as DoerGetter);

    assert_eq!(Arc::strong_count(&arc), 2);

    let doer = getter.dget_1();

    assert_eq!(Arc::strong_count(&arc), 3);

    let _ = getter.dget_1();

    assert_eq!(Arc::strong_count(&arc), 3);

    std::mem::drop(getter);

    assert_eq!(Arc::strong_count(&arc), 2);

    std::mem::drop(doer);

    assert_eq!(Arc::strong_count(&arc), 1);
}

#[test]
fn use_clone_obj() {
    let sa = SA {};

    let arc = std::sync::Arc::from(());

    assert_eq!(Arc::strong_count(&arc), 1);

    let opt_arc = CArc::<()>::from(arc.clone());

    assert_eq!(Arc::strong_count(&arc), 2);

    let obj: crate::ext::CloneBaseArcBox<_, _> = trait_obj!((sa, opt_arc) as Clone);

    assert_eq!(Arc::strong_count(&arc), 2);

    let cloned = obj.clone();

    assert_eq!(Arc::strong_count(&arc), 3);

    std::mem::drop(cloned);

    assert_eq!(Arc::strong_count(&arc), 2);

    std::mem::drop(obj);

    assert_eq!(Arc::strong_count(&arc), 1);
}
