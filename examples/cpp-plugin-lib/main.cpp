#include <stdio.h>
#include <string.h>
#include "bindings.h"
#include <vector>
#include <unordered_map>

struct KvStore {
	std::unordered_map<std::string, size_t> map;
};

template<typename T = CBox<KvStore>, typename C = COptArc<void>>
struct KvStoreContainer : FeaturesGroupContainer<T, C> {

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

auto main_feature_vtbl = MainFeatureVtblImpl<KvStoreContainer<>>();
auto main_feature_mut_vtbl = MainFeatureVtblImpl<KvStoreContainer<KvStore *>>();

auto kvstore_vtbl = KeyValueStoreVtblImpl<KvStoreContainer<>>();
auto kvstore_mut_vtbl = KeyValueStoreVtblImpl<KvStoreContainer<KvStore *>>();

struct PluginCPP {
	KvStore store;
};

template<typename T = CBox<PluginCPP>, typename C = COptArc<void>>
struct PluginCPPContainer : CGlueObjContainer<T, C, PluginInnerRetTmp<C>> {

	using Parent = CGlueObjContainer<T, C, PluginInnerRetTmp<C>>;

	PluginCPPContainer(T &&instance, C &&context) : Parent { instance, context } {}

	using BorrowedType = FeaturesGroup<CBox<void>, C>;
	using OwnedType = FeaturesGroup<CBox<void>, C>;
	using OwnedTypeMut = FeaturesGroup<void*, C>;

	static BorrowedType borrow_features(Parent *self) {
		BorrowedType ret;
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

auto plugin_vtbl = PluginInnerVtblImpl<PluginCPPContainer<>>(get_root_layout());

extern "C" PluginInnerBaseArcBox<PluginCPP, void> create_plugin(COptArc<void> &library) {
	PluginInnerBaseArcBox<PluginCPP, void> ret;

	ret.vtbl = &plugin_vtbl;
	ret.container = PluginCPPContainer<>(CBox<PluginCPP>::new_box(), library.clone());

	return ret;
}

