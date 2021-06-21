//! This is the main plugin API
//!
//! This crate is shared by plugins and users.

use cglue::prelude::v1::*;
use libloading::{library_filename, Library, Symbol};

#[cglue_trait]
pub trait PluginInner<'a> {
    #[wrap_with_group(FeaturesGroup)]
    type BorrowedType: MainFeature + 'a;
    #[wrap_with_group(FeaturesGroup)]
    type OwnedType: MainFeature + 'static;
    #[wrap_with_group_mut(FeaturesGroup)]
    type OwnedTypeMut: MainFeature + 'a;

    fn borrow_features(&'a mut self) -> Self::BorrowedType;

    fn into_features(self) -> Self::OwnedType;

    fn mut_features(&'a mut self) -> &'a mut Self::OwnedTypeMut;
}

/// Having the inner type with a lifetime allows to borrow features for any lifetime.
///
/// This could be avoided with [GAT](https://rust-lang.github.io/rfcs/1598-generic_associated_types.html)
pub trait Plugin: for<'a> PluginInner<'a> {}
impl<T: for<'a> PluginInner<'a>> Plugin for T {}

#[repr(C)]
pub struct KeyValue<'a>(pub CSliceRef<'a, u8>, pub usize);

pub type KeyValueCallback<'a> = OpaqueCallback<'a, KeyValue<'a>>;

#[cglue_trait]
#[cglue_forward]
pub trait MainFeature {
    fn print_self(&self);
}

#[cglue_trait]
#[cglue_forward]
pub trait KeyValueStore {
    fn write_key_value(&mut self, name: &str, val: usize);
    fn get_key_value(&self, name: &str) -> usize;
}

#[cglue_trait]
pub trait KeyValueDumper {
    fn dump_key_values<'a>(&'a self, callback: KeyValueCallback<'a>);
}

cglue_trait_group!(FeaturesGroup, {
    MainFeature
}, {
    KeyValueStore,
    KeyValueDumper,
    Clone
});

/// Load a plugin from a given library.
///
/// # Safety
///
/// Input library must implement a correct `create_plugin` function. Its signature must be as
/// follows:
///
/// `extern "C" fn(&COptArc<T>) -> PluginInnerArcBox<'static>`
///
/// Where `T` is any type, since it's opaque.
#[no_mangle]
pub unsafe extern "C" fn load_plugin(name: ReprCStr<'_>) -> PluginInnerArcBox<'static> {
    let mut current_exe = std::env::current_exe().unwrap();
    current_exe.set_file_name(library_filename(name.as_ref()));
    let lib = Library::new(current_exe).unwrap();
    let sym: Symbol<extern "C" fn(&COptArc<Library>) -> PluginInnerArcBox<'static>> =
        lib.get(b"create_plugin\0").unwrap();
    let sym = sym.into_raw();
    let arc = CArc::from(lib);
    sym(&Some(arc).into())
}
