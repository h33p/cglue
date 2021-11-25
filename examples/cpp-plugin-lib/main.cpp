// This is an example of a plugin implemented in C++

#include <stdio.h>
#include <string.h>
#include "bindings.h"
#include <vector>
#include <unordered_map>

// Data of the main features key value store.
struct KvStore {
	std::unordered_map<std::string, size_t> map;
};

// This is an implementation of the `KeyValueStore + MainFeature` traits.
//
// We inherit from `FeaturesGroupContainer` so that implementation
// functions can easily access the fields.
template<typename T = CBox<KvStore>, typename C = COptArc<void>>
struct KvStoreContainer : FeaturesGroupContainer<T, C> {

	// Parent type must always be defined to the class we inherit.
	// This parent is used to know which type the implementation is actually for.
	using Parent = FeaturesGroupContainer<T, C>;

	KvStoreContainer(T &&instance, C &&context) : Parent { instance, context } {}

	static void print_self(const Parent *self) {
		for (const auto &e : self->instance->map) {
			printf("%s: %zu\n", e.first.c_str(), e.second);
		}
	}

	static void write_key_value(Parent *self, CSliceRef<uint8_t> name, size_t val) {
		self->instance->map[name] = val;
	}

	static size_t get_key_value(const Parent *self, CSliceRef<uint8_t> name) {
		auto it = self->instance->map.find(name);
		if (it == self->instance->map.end()) {
			return 0;
		} else {
			return it->second;
		}
	}
};

// This is the actual object that gets boxed.
//
// Use it like a normal class/struct.
struct PluginCPP {
	KvStore store;
};

// This contains implementation for `PluginInner` trait.
template<typename T = CBox<PluginCPP>, typename C = COptArc<void>>
struct PluginCPPContainer : CGlueObjContainer<T, C, PluginInnerRetTmp<C>> {

	// Vtables inserted when borrowing or converting into features.
	static constexpr MainFeatureVtblImpl<KvStoreContainer<>> main_feature_vtbl {};
	static constexpr MainFeatureVtblImpl<KvStoreContainer<KvStore *>> main_feature_mut_vtbl {};
	static constexpr KeyValueStoreVtblImpl<KvStoreContainer<>> kvstore_vtbl {};
	static constexpr KeyValueStoreVtblImpl<KvStoreContainer<KvStore *>> kvstore_mut_vtbl {};

	// The same as in KvStoreContainer, define the parent.
	using Parent = CGlueObjContainer<T, C, PluginInnerRetTmp<C>>;

	// Initialize the underlying container with instance and context objects.
	PluginCPPContainer(T &&instance, C &&context) : Parent { instance, context } {}

	using BorrowedType = FeaturesGroup<CBox<void>, C>;
	using OwnedType = FeaturesGroup<CBox<void>, C>;
	using OwnedTypeMut = FeaturesGroup<void*, C>;

	static BorrowedType borrow_features(Parent *self) {
		BorrowedType ret;
		// Need to manually opaquify the vtables (won't be needed in the future).
		ret.vtbl_mainfeature = (decltype(ret.vtbl_mainfeature))&main_feature_vtbl;
		ret.vtbl_keyvaluestore = (decltype(ret.vtbl_keyvaluestore))&kvstore_vtbl;
		ret.container.instance = (CBox<void>)CBox<KvStore>(&self->instance.instance->store);
		return ret;
	}

	static OwnedType into_features(Parent self) {
		OwnedType ret;
		ret.vtbl_mainfeature = (decltype(ret.vtbl_mainfeature))&main_feature_vtbl;
		ret.vtbl_keyvaluestore = (decltype(ret.vtbl_keyvaluestore))&kvstore_vtbl;
		ret.container.instance = CBox<KvStore>(std::move(self.instance.instance->store));
		return ret;
	}

	static OwnedTypeMut *mut_features(Parent *self) {
		OwnedTypeMut *ret = &self->ret_tmp.assume_init().mut_features;
		ret->vtbl_mainfeature = (decltype(ret->vtbl_mainfeature))&main_feature_mut_vtbl;
		ret->vtbl_keyvaluestore = (decltype(ret->vtbl_keyvaluestore))&kvstore_mut_vtbl;
		ret->container.instance = self->instance.instance;
		return ret;
	}
};

PluginInnerVtblImpl<PluginCPPContainer<>> plugin_vtbl;

extern "C" {
	// Create a new plugin ArcBox object that clones the library into itself.
	PluginInnerBaseArcBox<PluginCPP, void> create_plugin(COptArc<void> &library) {
		PluginInnerBaseArcBox<PluginCPP, void> ret;

		ret.vtbl = &plugin_vtbl;
		ret.container = PluginCPPContainer<>(CBox<PluginCPP>::new_box(), library.clone());

		return ret;
	}

	// Define the header and opaque cast the plugin creation function.
	PluginHeader PLUGIN_HEADER { ROOT_LAYOUT, (decltype(PLUGIN_HEADER.create))create_plugin };
}
