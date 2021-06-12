use super::structs::*;
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
    fn do_thing(&self) {}
}

#[test]
fn use_plugin() {}
