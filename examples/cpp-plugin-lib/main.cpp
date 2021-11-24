#include <stdio.h>
#include <string.h>
#include "bindings.h"
#include <vector>
#include <unordered_map>

#define CGLUE_CONT_TY(cont, inst, trait, ctx) cont##Container<inst, ctx, trait##RetTmp<ctx>>
#define GROUP_CONT_TY(cont, inst, trait, ctx) cont##Container<inst, ctx>
#define VTBL_TY(MODE, cont, trait, inst, ctx) trait##Vtbl<MODE##_CONT_TY(cont, inst, trait, ctx)>

#define VTBL_BUILDER(MODE, cont, impl_cont, trait, ...) \
	template<typename T, typename C> \
	VTBL_TY(MODE, cont, trait, T, C) __construct_##cont##trait () { \
		using Cont = impl_cont##Container<T, C>; \
		return __VA_ARGS__; \
	}

#define DEFINE_VTBL(MODE, name, cont, trait, inst, ctx) \
	VTBL_TY(MODE, cont, trait, inst, ctx) name = __construct_##cont##trait <inst, ctx> ()

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

VTBL_BUILDER(GROUP, FeaturesGroup, KvStore, MainFeature, {
	nullptr,
	&Cont::print_self,
});

DEFINE_VTBL(GROUP, main_feature, FeaturesGroup, MainFeature, CBox<KvStore>, COptArc<void>);
DEFINE_VTBL(GROUP, main_feature_mut, FeaturesGroup, MainFeature, KvStore *, COptArc<void>);

VTBL_BUILDER(GROUP, FeaturesGroup, KvStore, KeyValueStore, {
	nullptr,
	&Cont::write_key_value,
	&Cont::get_key_value,
});

DEFINE_VTBL(GROUP, kvstore, FeaturesGroup, KeyValueStore, CBox<KvStore>, COptArc<void>);
DEFINE_VTBL(GROUP, kvstore_mut, FeaturesGroup, KeyValueStore, KvStore *, COptArc<void>);

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
		ret.vtbl_mainfeature = (decltype(ret.vtbl_mainfeature))&main_feature;
		ret.vtbl_keyvaluestore = (decltype(ret.vtbl_keyvaluestore))&kvstore;
		ret.container.instance = (CBox<void>)CBox<KvStore>(&self->instance.instance->store);
		return ret;
	}

	static OwnedType into_features(Parent self) {
		OwnedType ret;
		ret.vtbl_mainfeature = (decltype(ret.vtbl_mainfeature))&main_feature;
		ret.vtbl_keyvaluestore = (decltype(ret.vtbl_keyvaluestore))&kvstore;
		ret.container.instance = CBox<KvStore>(std::move(self.instance.instance->store));
		return ret;
	}

	static OwnedTypeMut *mut_features(Parent *self) {
		OwnedTypeMut *ret = &self->ret_tmp.assume_init().mut_features;
		ret->vtbl_mainfeature = (decltype(ret->vtbl_mainfeature))&main_feature_mut;
		ret->vtbl_keyvaluestore = (decltype(ret->vtbl_keyvaluestore))&kvstore_mut;
		ret->container.instance = self->instance.instance;
		return ret;
	}
};

VTBL_BUILDER(CGLUE, CGlueObj, PluginCPP, PluginInner, {
	get_root_layout(),
	&Cont::borrow_features,
	&Cont::into_features,
	&Cont::mut_features,
} );

DEFINE_VTBL(CGLUE, plugin_vtbl, CGlueObj, PluginInner, CBox<PluginCPP>, COptArc<void>);

extern "C" PluginInnerBaseArcBox<PluginCPP, void> create_plugin(COptArc<void> &library) {
	PluginInnerBaseArcBox<PluginCPP, void> ret;

	ret.vtbl = &plugin_vtbl;
	ret.container = (PluginCPPContainer<>::Parent)PluginCPPContainer<>(CBox<PluginCPP>::new_box(), library.clone());

	return ret;
}

