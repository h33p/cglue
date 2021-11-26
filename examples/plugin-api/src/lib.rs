//! This is the main plugin API
//!
//! This crate is shared by plugins and users.

pub use abi_stable::type_layout::TypeLayout;
use abi_stable::StableAbi;
use cglue::prelude::v1::{trait_group::compare_layouts, *};
use core::mem::MaybeUninit;
use core::num::NonZeroI32;
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
#[derive(::abi_stable::StableAbi)]
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
    fn print_ints(&self, iter: CIterator<i32>);
}

cglue_trait_group!(FeaturesGroup, {
    MainFeature
}, {
    KeyValueStore,
    KeyValueDumper,
    Clone
});

/// Describes possible errors that can occur loading the library
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Error {
    Path = 1,
    Loading = 2,
    Symbol = 3,
    Abi = 4,
}

impl IntError for Error {
    fn into_int_err(self) -> NonZeroI32 {
        NonZeroI32::new(self as u8 as _).unwrap()
    }

    fn from_int_err(err: NonZeroI32) -> Self {
        match err.get() {
            1 => Self::Path,
            2 => Self::Loading,
            3 => Self::Symbol,
            4 => Self::Abi,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

/// Plugin header that the API looks for.
///
/// Plugins should define the header with name `PLUGIN_HEADER` with no mangling.
#[repr(C)]
pub struct PluginHeader {
    pub layout: &'static TypeLayout,
    pub create: extern "C" fn(&CArc<cglue::trait_group::c_void>) -> PluginInnerArcBox<'static>,
}

/// Load a plugin from a given library.
///
/// # Safety
///
/// Input library must implement a correct `create_plugin` and `get_root_layout()` functions.
/// Its signatures must be as follows:
///
/// `extern "C" fn crate_plugin(&CArc<T>) -> PluginInnerArcBox<'static>`
/// `extern "C" fn get_root_layout() -> Option<&'static TypeLayout>`
///
/// Where `T` is any type, since it's opaque. Meanwhile, `get_root_layout` should simply
/// [call the one in this crate](self::get_root_layout). It is used to verify
/// version mismatches.
#[no_mangle]
pub unsafe extern "C" fn load_plugin(
    name: ReprCStr<'_>,
    ok_out: &mut MaybeUninit<PluginInnerArcBox<'static>>,
) -> i32 {
    load_plugin_impl(name.as_ref()).into_int_out_result(ok_out)
}

unsafe fn load_plugin_impl(name: &str) -> Result<PluginInnerArcBox<'static>, Error> {
    let mut current_exe = std::env::current_exe().map_err(|_| Error::Path)?;
    current_exe.set_file_name(library_filename(name));
    let lib = Library::new(current_exe).map_err(|e| {
        println!("{}", e);
        Error::Loading
    })?;

    let header: Symbol<&'static PluginHeader> = lib.get(b"PLUGIN_HEADER\0").map_err(|e| {
        println!("{}", e);
        Error::Symbol
    })?;
    let header = header.into_raw();

    if !compare_layouts(Some(ROOT_LAYOUT), Some(header.layout)).is_valid_strict() {
        return Err(Error::Abi);
    }

    let arc = CArc::from(lib);
    Ok((header.create)(&arc.into_opaque()))
}

/// Layout for the root vtable.
///
/// Layout that should be embedded to a `PluginHeader`.
/// Other layouts are not necessary, because the very root depends on them already.
#[no_mangle]
pub static ROOT_LAYOUT: &TypeLayout = PluginInnerArcBox::LAYOUT;
