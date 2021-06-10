use super::simple::structs::*;
use crate::arc::*;
use crate::*;
use std::sync::Arc;

#[cglue_arc_wrappable]
#[cglue_trait]
pub trait DoThings {
    fn dt_1(&self) -> usize;
}

impl DoThings for SA {
    fn dt_1(&self) -> usize {
        55
    }
}

#[cglue_arc_wrappable]
pub trait DoThingsAssoc {
    type ReturnType;

    fn dta_1(&self) -> Self::ReturnType;
}

impl DoThingsAssoc for SA {
    type ReturnType = usize;

    fn dta_1(&self) -> Self::ReturnType {
        56
    }
}

#[cglue_arc_wrappable]
pub trait DoThingsAssocWrapped {
    #[arc_wrap]
    type ReturnType;

    fn dtaw_1(&self) -> Self::ReturnType;
}

impl DoThingsAssocWrapped for SA {
    type ReturnType = usize;

    fn dtaw_1(&self) -> Self::ReturnType {
        57
    }
}

#[cglue_arc_wrappable]
#[cglue_trait]
pub trait DoerGetter {
    #[arc_wrap]
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

#[test]
fn use_dothings() {
    let sa = SA {};
    let wrapped: ArcWrapped<SA, ()> = (sa, Arc::new(())).into();
    assert_eq!(wrapped.dt_1(), 55);
}

#[test]
fn use_dothings_assoc() {
    let sa = SA {};
    let wrapped: ArcWrapped<SA, ()> = (sa, Arc::new(())).into();
    assert_eq!(wrapped.dta_1(), 56);
}

#[test]
fn use_dothings_assoc_wrapped() {
    let sa = SA {};
    let wrapped: ArcWrapped<SA, ()> = (sa, Arc::new(())).into();
    assert_eq!(wrapped.dtaw_1().inner, 57);
}

#[test]
fn use_getter_obj() {
    let sa = SA {};

    let arc = std::sync::Arc::from(());

    assert_eq!(Arc::strong_count(&arc), 1);

    let opt_arc = COptArc::from(Some(CArc::from(arc.clone())));

    assert_eq!(Arc::strong_count(&arc), 2);

    let wrapped: ArcWrapped<SA, ()> = (sa, opt_arc).into();

    let getter = trait_obj!(wrapped as DoerGetter);

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

    let opt_arc = COptArc::from(Some(CArc::from(arc.clone())));

    assert_eq!(Arc::strong_count(&arc), 2);

    let wrapped: ArcWrapped<SA, ()> = (sa, opt_arc).into();

    assert_eq!(Arc::strong_count(&arc), 2);

    let obj = trait_obj!(wrapped as Clone);

    assert_eq!(Arc::strong_count(&arc), 2);

    let cloned = obj.clone();

    assert_eq!(Arc::strong_count(&arc), 3);

    std::mem::drop(cloned);

    assert_eq!(Arc::strong_count(&arc), 2);

    std::mem::drop(obj);

    assert_eq!(Arc::strong_count(&arc), 1);
}

#[test]
fn use_debug_obj() {
    let sa = SA {};

    let arc = std::sync::Arc::from(());

    assert_eq!(Arc::strong_count(&arc), 1);

    let opt_arc = COptArc::from(Some(CArc::from(arc.clone())));

    assert_eq!(Arc::strong_count(&arc), 2);

    let wrapped: ArcWrapped<SA, ()> = (sa, opt_arc).into();

    assert_eq!(Arc::strong_count(&arc), 2);

    let obj = trait_obj!(wrapped as Debug);

    assert_eq!(&format!("{:?}", obj), "SA");
}
