//! Example plugin library.
//!
//! This plugin crate will not be known to the user, both parties will interact with the help of
//! the shared plugin API.

use cglue::prelude::v1::*;
use plugin_api::*;
use std::collections::HashMap;

#[derive(Default)]
struct KvRoot {
    store: KvStore,
}

impl<'a> PluginInner<'a> for KvRoot {
    type BorrowedType = Fwd<&'a mut KvStore>;
    type OwnedType = KvStore;
    type OwnedTypeMut = KvStore;

    fn borrow_features(&'a mut self) -> Self::BorrowedType {
        self.store.forward_mut()
    }

    fn into_features(self) -> Self::OwnedType {
        self.store
    }

    fn mut_features(&'a mut self) -> &'a mut Self::OwnedTypeMut {
        &mut self.store
    }
}

#[derive(Debug, Default, Clone)]
struct KvStore {
    map: HashMap<String, usize>,
}

impl MainFeature for KvStore {
    fn print_self(&self) {
        println!("{:?}", self.map);
    }
}

impl KeyValueStore for KvStore {
    fn write_key_value(&mut self, name: &str, val: usize) {
        self.map.insert(name.to_string(), val);
    }

    fn get_key_value(&self, name: &str) -> usize {
        self.map.get(name).copied().unwrap_or(0)
    }
}

impl KeyValueDumper for KvStore {
    fn dump_key_values<'a>(&'a self, callback: KeyValueCallback<'a>) {
        self.map
            .iter()
            .map(|(k, v)| KeyValue(k.as_str().into(), *v))
            .feed_into(callback);
    }

    fn print_ints(&self, iter: CIterator<i32>) {
        for (cnt, i) in iter.enumerate() {
            println!("{}: {}", cnt, i);
        }
    }
}

cglue_impl_group!(KvStore, FeaturesGroup,
// Owned `KvStore` has these types
{
    KeyValueStore,
    KeyValueDumper,
},
// The forward type can not be cloned, and KeyValueDumper is not implemented
{
    KeyValueStore,
});

extern "C" fn create_plugin(lib: &CArc<cglue::trait_group::c_void>) -> PluginInnerArcBox<'static> {
    trait_obj!((KvRoot::default(), lib.clone()) as PluginInner)
}

#[no_mangle]
pub static PLUGIN_HEADER: PluginHeader = PluginHeader {
    layout: ROOT_LAYOUT,
    create: create_plugin,
};
