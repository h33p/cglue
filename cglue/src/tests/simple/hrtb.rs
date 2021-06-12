use super::structs::*;
use super::trait_defs::*;
use crate::*;

pub trait Plugin: for<'a> PluginInner<'a> {}

#[cglue_trait]
pub trait PluginInner<'a> {
    #[wrap_with_obj(SubPlugin)]
    type Ret: SubPlugin + 'a;

    fn get_plug(&'a mut self) -> Self::Ret;
}

#[cglue_trait]
pub trait SubPlugin {
    fn do_thing(&self);
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

impl<'a> SubPlugin for Printer<'a> {
    fn do_thing(&self) {
        println!("{}", self.sa.ta_1());
    }
}

cglue_trait_group!(PluginInstance<'a>, { PluginInner<'a> }, { Clone });

cglue_impl_group!(SA, PluginInstance<'a>, {});

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

    let base = PluginInstance::from(&mut sa);

    let mut obj = crate::trait_group::Opaquable::into_opaque(base);

    let printer = obj.get_plug();

    printer.do_thing();
}
