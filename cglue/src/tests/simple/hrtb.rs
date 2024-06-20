use super::structs::*;
use super::trait_defs::*;
use crate::*;

pub trait Plugin: for<'a> PluginInner<'a> {}

#[cglue_trait]
pub trait PluginInner<'a> {
    #[wrap_with_obj(SubPlugin)]
    type Ret: SubPlugin<'a> + 'a;

    fn get_plug(&'a mut self) -> Self::Ret;
}

#[cglue_trait]
pub trait SubPlugin<'a> {
    #[wrap_with_obj_mut(TA)]
    type BorrowedTA: TA + 'a;

    fn do_thing(&self);
    fn get_ta(&'a mut self) -> &'a mut Self::BorrowedTA;
}

impl<'a> PluginInner<'a> for SA {
    type Ret = Printer<'a>;

    fn get_plug(&'a mut self) -> Self::Ret {
        Printer { sa: self }
    }
}

pub struct Printer<'a> {
    sa: &'a mut SA,
}

impl<'a, 'b> SubPlugin<'a> for Printer<'b> {
    type BorrowedTA = SA;

    fn do_thing(&self) {
        println!("{}", self.sa.ta_1());
    }

    fn get_ta(&mut self) -> &mut Self::BorrowedTA {
        self.sa
    }
}

#[cglue_trait]
pub trait AsSubThing {
    #[wrap_with_obj_mut(TA)]
    type SubTarget: TA;

    fn get_ta(&mut self) -> &mut Self::SubTarget;
}

#[repr(transparent)]
pub struct FwdMut<'a, T>(&'a mut T);

impl<'a, T: TA> TA for FwdMut<'a, T> {
    extern "C" fn ta_1(&self) -> usize {
        self.0.ta_1()
    }
}

pub struct Plug<T> {
    val: T,
}

impl<T: TA> AsSubThing for Plug<T> {
    type SubTarget = T;

    fn get_ta(&mut self) -> &mut Self::SubTarget {
        &mut self.val
    }
}

cglue_trait_group!(PluginInstance<'a>, { PluginInner<'a> }, { Clone });

cglue_impl_group!(SA, PluginInstance<'a>, {});

#[test]
fn use_subthing() {
    let mut sa = SA {};
    let val = FwdMut(&mut sa);

    let mut plug = Plug { val };

    let mut obj = trait_obj!(&mut plug as AsSubThing);
    obj.get_ta();
}

#[test]
fn build_subplugin() {
    let mut sa = SA {};
    let mut subplug = Printer { sa: &mut sa };

    let mut obj = trait_obj!(&mut subplug as SubPlugin);

    let ta = obj.get_ta();

    assert_eq!(ta.ta_1(), 5);
}

#[test]
fn use_plugin() {
    let sa = SA {};

    let mut obj = trait_obj!(sa as PluginInner);

    let printer = obj.get_plug();

    printer.do_thing();
}

#[test]
fn use_plugin_mut() {
    let mut sa = SA {};

    let mut obj = trait_obj!(&mut sa as PluginInner);

    let printer = obj.get_plug();

    printer.do_thing();
}

#[test]
fn use_plugin_group() {
    let sa = SA {};

    let mut obj = group_obj!(sa as PluginInstance);

    let printer = obj.get_plug();

    printer.do_thing();
}

#[test]
fn use_plugin_group_mut() {
    let mut sa = SA {};

    let base = PluginInstance::<_, _>::from(&mut sa);

    let mut obj = crate::trait_group::Opaquable::into_opaque(base);

    let printer = obj.get_plug();

    printer.do_thing();
}
