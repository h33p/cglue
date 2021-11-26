#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

// Construct a typed slice for rust functions
#define REF_SLICE(ty, buf, len) ((struct CSliceRef_##ty){(buf), (len)})

// Constructs a typed mutable slice for rust functions
#define MUT_SLICE(ty, buf, len) ((struct CSliceMut_##ty){(buf), (len)})

// Constructs a slice from a string for rust functions
// Note that strlen() is optimized out for string literals here
#define STR(string) \
    REF_SLICE(u8, (const unsigned char *)string, strlen(string))

// Constructs a callback
#define CALLBACK(ty, ctx, func) \
    (struct Callback_c_void__##ty){(ctx), (bool (*)(void *, ty))(func)}

// Constructs a dynamic collect callback
//
// This callback will collect all elements into a buffer accessible within `(*name_data)`.
// It is the same buffer as `name_base.buf`, but cast into the correct type. The buffer must
// be freed with `free(3)`.
//
// Number of elements is accessible within `name_base.size`, alongside its capacity.
//
// After creation, this callback should not exit its scope.
#define COLLECT_CB(ty, name) \
    struct CollectBase name##_base = {}; \
    ty **name##_data = (ty **)&name##_base.buf; \
    Callback_c_void__##ty name = CALLBACK(ty, &name##_base, cb_collect_dynamic_##ty)

// Constructs a static collect callback
//
// This callback will collect all elements into the provided buffer up to given length.
//
// Any additional elements that do not fit will be skipped.
//
// Number of elements is accessible within `name_base.size`.
//
// After creation, this callback should not exit its scope.
#define COLLECT_CB_INTO(ty, name, data, len) \
    struct CollectBase name##_base = (struct CollectBase){ (void *)data, (size_t)len, 0 }; \
    ty **name##_data = (ty **)&name##_base.buf; \
    Callback_c_void__##ty name = CALLBACK(ty, &name##_base, cb_collect_static_##ty)

// Constructs a static collect callback (for arrays)
//
// This is the same as `COLLECT_CB_INTO`, but performs an automatic array size calculation.
//
// Number of elements is accessible within `name_base.size`.
//
// After creation, this callback should not exit its scope.
#define COLLECT_CB_INTO_ARR(ty, name, data) \
    COLLECT_CB_INTO(ty, name, data, sizeof(data) / sizeof(*data))

// Constructs a count callback
//
// This callback will simply count the number of elements encountered, and this value is
// accessible through `name_count` variable.
//
// After creation, this callback should not exit its scope.
#define COUNT_CB(ty, name) \
    size_t name##_count = 0; \
    Callback_c_void__##ty name = CALLBACK(ty, &name##_count, cb_count_##ty)

#define BUF_ITER_SPEC(ty, ty2, name, buf, len) \
    struct BufferIterator name##_base = (struct BufferIterator){(const void *)(const ty2 *)buf, len, 0, sizeof(ty2)}; \
    CIterator_##ty name = (CIterator_##ty){ &name##_base, (int32_t (*)(void *, ty2 *))buf_iter_next }

#define BUF_ITER_ARR_SPEC(ty, ty2, name, buf) BUF_ITER_SPEC(ty, ty2, name, buf, sizeof(buf) / sizeof(*buf))

#define BUF_ITER(ty, name, buf, len) \
    BUF_ITER_SPEC(ty, ty, name, buf, len)

#define BUF_ITER_ARR(ty, name, buf) BUF_ITER(ty, name, buf, sizeof(buf) / sizeof(*buf))

// Forward declarations for vtables and their wrappers
struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void;
struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void;
struct FeaturesGroup_CBox_c_void_____CArc_c_void;
struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
struct FeaturesGroup_CBox_c_void_____CArc_c_void;
struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
struct FeaturesGroup_CBox_c_void_____CArc_c_void;
struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
struct FeaturesGroup_CBox_c_void_____CArc_c_void;
struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
struct FeaturesGroup_____c_void__CArc_c_void;
struct FeaturesGroupContainer_____c_void__CArc_c_void;
struct FeaturesGroup_____c_void__CArc_c_void;
struct FeaturesGroupContainer_____c_void__CArc_c_void;
struct FeaturesGroup_____c_void__CArc_c_void;
struct FeaturesGroupContainer_____c_void__CArc_c_void;
struct FeaturesGroup_____c_void__CArc_c_void;
struct FeaturesGroupContainer_____c_void__CArc_c_void;

typedef struct TypeLayout TypeLayout;

/**
 * Wrapper around null-terminated C-style strings.
 *
 * Analog to Rust's `str`, [`ReprCStr`] borrows the underlying data.
 */
typedef const char *ReprCStr;

/**
 * FFI-safe box
 *
 * This box has a static self reference, alongside a custom drop function.
 *
 * The drop function can be called from anywhere, it will free on correct allocator internally.
 */
typedef struct CBox_c_void {
    void *instance;
    void (*drop_fn)(void*);
} CBox_c_void;
typedef struct CArc_c_void {
    const void *instance;
    const void *(*clone_fn)(const void*);
    void (*drop_fn)(const void*);
} CArc_c_void;

typedef struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void {
    struct CBox_c_void instance;
    CArc_c_void context;
} FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
/**
 * CGlue vtable for trait MainFeature.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct MainFeatureVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void {
    void (*print_self)(const struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont);
} MainFeatureVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
/**
 * CGlue vtable for trait Clone.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct CloneVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void {
    struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void (*clone)(const struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont);
} CloneVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
/**
 * Wrapper around const slices.
 *
 * This is meant as a safe type to pass across the FFI boundary with similar semantics as regular
 * slice. However, not all functionality is present, use the slice conversion functions.
 *
 * # Examples
 *
 * Simple conversion:
 *
 * ```
 * use cglue::slice::CSliceRef;
 *
 * let arr = [0, 5, 3, 2];
 *
 * let cslice = CSliceRef::from(&arr[..]);
 *
 * let slice = cslice.as_slice();
 *
 * assert_eq!(&arr, slice);
 * ```
 */
typedef struct CSliceRef_u8 {
    const uint8_t *data;
    uintptr_t len;
} CSliceRef_u8;

typedef struct KeyValue {
    struct CSliceRef_u8 _0;
    uintptr_t _1;
} KeyValue;

typedef struct Callback_c_void__KeyValue {
    void *context;
    bool (*func)(void*, struct KeyValue);
} Callback_c_void__KeyValue;

typedef struct Callback_c_void__KeyValue OpaqueCallback_KeyValue;

typedef OpaqueCallback_KeyValue KeyValueCallback;

/**
 * FFI compatible iterator.
 *
 * Any mutable reference to an iterator can be converted to a `CIterator`.
 *
 * `CIterator<T>` implements `Iterator<Item = T>`.
 *
 * # Examples
 *
 * Using [`AsCIterator`](AsCIterator) helper:
 *
 * ```
 * use cglue::iter::{CIterator, AsCIterator};
 *
 * extern "C" fn sum_all(iter: CIterator<usize>) -> usize {
 *     iter.sum()
 * }
 *
 * let mut iter = (0..10).map(|v| v * v);
 *
 * assert_eq!(sum_all(iter.as_citer()), 285);
 * ```
 *
 * Converting with `Into` trait:
 *
 * ```
 * use cglue::iter::{CIterator, AsCIterator};
 *
 * extern "C" fn sum_all(iter: CIterator<usize>) -> usize {
 *     iter.sum()
 * }
 *
 * let mut iter = (0..=10).map(|v| v * v);
 *
 * assert_eq!(sum_all((&mut iter).into()), 385);
 * ```
 */
typedef struct CIterator_i32 {
    void *iter;
    int32_t (*func)(void*, int32_t *out);
} CIterator_i32;

/**
 * CGlue vtable for trait KeyValueDumper.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct KeyValueDumperVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void {
    void (*dump_key_values)(const struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont, KeyValueCallback callback);
    void (*print_ints)(const struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont, struct CIterator_i32 iter);
} KeyValueDumperVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
/**
 * CGlue vtable for trait KeyValueStore.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct KeyValueStoreVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void {
    void (*write_key_value)(struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont, struct CSliceRef_u8 name, uintptr_t val);
    uintptr_t (*get_key_value)(const struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void *cont, struct CSliceRef_u8 name);
} KeyValueStoreVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void;
/**
 * Trait group potentially implementing `MainFeature < > + :: cglue :: ext :: core :: clone :: Clone < > + KeyValueDumper < > + KeyValueStore < >` traits.
 *
 * Optional traits are not implemented here, however. There are numerous conversion
 * functions available for safely retrieving a concrete collection of traits.
 *
 * `check_impl_` functions allow to check if the object implements the wanted traits.
 *
 * `into_impl_` functions consume the object and produce a new final structure that
 * keeps only the required information.
 *
 * `cast_impl_` functions merely check and transform the object into a type that can
 *be transformed back into `FeaturesGroup` without losing data.
 *
 * `as_ref_`, and `as_mut_` functions obtain references to safe objects, but do not
 * perform any memory transformations either. They are the safest to use, because
 * there is no risk of accidentally consuming the whole object.
 */
typedef struct FeaturesGroup_CBox_c_void_____CArc_c_void {
    const struct MainFeatureVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void *vtbl_mainfeature;
    const struct CloneVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void *vtbl_clone;
    const struct KeyValueDumperVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void *vtbl_keyvaluedumper;
    const struct KeyValueStoreVtbl_FeaturesGroupContainer_CBox_c_void_____CArc_c_void *vtbl_keyvaluestore;
    struct FeaturesGroupContainer_CBox_c_void_____CArc_c_void container;
} FeaturesGroup_CBox_c_void_____CArc_c_void;

// Typedef for default container and context type
typedef struct FeaturesGroup_CBox_c_void_____CArc_c_void FeaturesGroup;
/**
 * FFI-Safe Arc
 *
 * This is an FFI-Safe equivalent of Arc<T> and Option<Arc<T>>.
 */

typedef struct FeaturesGroupContainer_____c_void__CArc_c_void {
    void *instance;
    struct CArc_c_void context;
} FeaturesGroupContainer_____c_void__CArc_c_void;

/**
 * CGlue vtable for trait MainFeature.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct MainFeatureVtbl_FeaturesGroupContainer_____c_void__CArc_c_void {
    void (*print_self)(const struct FeaturesGroupContainer_____c_void__CArc_c_void *cont);
} MainFeatureVtbl_FeaturesGroupContainer_____c_void__CArc_c_void;

/**
 * CGlue vtable for trait Clone.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct CloneVtbl_FeaturesGroupContainer_____c_void__CArc_c_void {
    struct FeaturesGroupContainer_____c_void__CArc_c_void (*clone)(const struct FeaturesGroupContainer_____c_void__CArc_c_void *cont);
} CloneVtbl_FeaturesGroupContainer_____c_void__CArc_c_void;

/**
 * CGlue vtable for trait KeyValueDumper.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct KeyValueDumperVtbl_FeaturesGroupContainer_____c_void__CArc_c_void {
    void (*dump_key_values)(const struct FeaturesGroupContainer_____c_void__CArc_c_void *cont, KeyValueCallback callback);
    void (*print_ints)(const struct FeaturesGroupContainer_____c_void__CArc_c_void *cont, struct CIterator_i32 iter);
} KeyValueDumperVtbl_FeaturesGroupContainer_____c_void__CArc_c_void;

/**
 * CGlue vtable for trait KeyValueStore.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct KeyValueStoreVtbl_FeaturesGroupContainer_____c_void__CArc_c_void {
    void (*write_key_value)(struct FeaturesGroupContainer_____c_void__CArc_c_void *cont, struct CSliceRef_u8 name, uintptr_t val);
    uintptr_t (*get_key_value)(const struct FeaturesGroupContainer_____c_void__CArc_c_void *cont, struct CSliceRef_u8 name);
} KeyValueStoreVtbl_FeaturesGroupContainer_____c_void__CArc_c_void;

/**
 * Trait group potentially implementing `MainFeature < > + :: cglue :: ext :: core :: clone :: Clone < > + KeyValueDumper < > + KeyValueStore < >` traits.
 *
 * Optional traits are not implemented here, however. There are numerous conversion
 * functions available for safely retrieving a concrete collection of traits.
 *
 * `check_impl_` functions allow to check if the object implements the wanted traits.
 *
 * `into_impl_` functions consume the object and produce a new final structure that
 * keeps only the required information.
 *
 * `cast_impl_` functions merely check and transform the object into a type that can
 *be transformed back into `FeaturesGroup` without losing data.
 *
 * `as_ref_`, and `as_mut_` functions obtain references to safe objects, but do not
 * perform any memory transformations either. They are the safest to use, because
 * there is no risk of accidentally consuming the whole object.
 */
typedef struct FeaturesGroup_____c_void__CArc_c_void {
    const struct MainFeatureVtbl_FeaturesGroupContainer_____c_void__CArc_c_void *vtbl_mainfeature;
    const struct CloneVtbl_FeaturesGroupContainer_____c_void__CArc_c_void *vtbl_clone;
    const struct KeyValueDumperVtbl_FeaturesGroupContainer_____c_void__CArc_c_void *vtbl_keyvaluedumper;
    const struct KeyValueStoreVtbl_FeaturesGroupContainer_____c_void__CArc_c_void *vtbl_keyvaluestore;
    struct FeaturesGroupContainer_____c_void__CArc_c_void container;
} FeaturesGroup_____c_void__CArc_c_void;

/**
 * Temporary return value structure, for returning wrapped references.
 *
 * This structure contains data for each vtable function that returns a reference to
 * an associated type. Note that these temporary values should not be accessed
 * directly. Use the trait functions.
 */
typedef struct PluginInnerRetTmp_CArc_c_void {
    struct FeaturesGroup_____c_void__CArc_c_void mut_features;
} PluginInnerRetTmp_CArc_c_void;

/**
 * Simple CGlue trait object container.
 *
 * This is the simplest form of container, represented by an instance, clone context, and
 * temporary return context.
 *
 * `instance` value usually is either a reference, or a mutable reference, or a `CBox`, which
 * contains static reference to the instance, and a dedicated drop function for freeing resources.
 *
 * `context` is either `PhantomData` representing nothing, or typically a `CArc` that can be
 * cloned at will, reference counting some resource, like a `Library` for automatic unloading.
 *
 * `ret_tmp` is usually `PhantomData` representing nothing, unless the trait has functions that
 * return references to associated types, in which case space is reserved for wrapping structures.
 */
typedef struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void {
    struct CBox_c_void instance;
    struct CArc_c_void context;
    struct PluginInnerRetTmp_CArc_c_void ret_tmp;
} CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void;

/**
 * CGlue vtable for trait PluginInner.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
typedef struct PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void {
    struct FeaturesGroup_CBox_c_void_____CArc_c_void (*borrow_features)(struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void *cont);
    struct FeaturesGroup_CBox_c_void_____CArc_c_void (*into_features)(struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void cont);
    struct FeaturesGroup_____c_void__CArc_c_void *(*mut_features)(struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void *cont);
} PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void;

/**
 * Simple CGlue trait object.
 *
 * This is the simplest form of CGlue object, represented by a container and vtable for a single
 * trait.
 *
 * Container merely is a this pointer with some optional temporary return reference context.
 */
typedef struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void {
    const struct PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void *vtbl;
    struct CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void container;
} CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void;

// Typedef for default container and context type
typedef struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void PluginInner;
/**
 * Base CGlue trait object for trait PluginInner.
 */
typedef struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void PluginInnerBase_CBox_c_void_____CArc_c_void;
/**
 * CtxBoxed CGlue trait object for trait PluginInner with context.
 */
typedef PluginInnerBase_CBox_c_void_____CArc_c_void PluginInnerBaseCtxBox_c_void__CArc_c_void;
/**
 * Boxed CGlue trait object for trait PluginInner with a [`CArc`](cglue::arc::CArc) reference counted context.
 */
typedef PluginInnerBaseCtxBox_c_void__CArc_c_void PluginInnerBaseArcBox_c_void__c_void;
/**
 * Opaque Boxed CGlue trait object for trait PluginInner with a [`CArc`](cglue::arc::CArc) reference counted context.
 */
typedef PluginInnerBaseArcBox_c_void__c_void PluginInnerArcBox;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

extern const TypeLayout *ROOT_LAYOUT;

/**
 * Load a plugin from a given library.
 *
 * # Safety
 *
 * Input library must implement a correct `create_plugin` and `get_root_layout()` functions.
 * Its signatures must be as follows:
 *
 * `extern "C" fn crate_plugin(&CArc<T>) -> PluginInnerArcBox<'static>`
 * `extern "C" fn get_root_layout() -> Option<&'static TypeLayout>`
 *
 * Where `T` is any type, since it's opaque. Meanwhile, `get_root_layout` should simply
 * [call the one in this crate](self::get_root_layout). It is used to verify
 * version mismatches.
 */
int32_t load_plugin(ReprCStr name, PluginInnerArcBox *ok_out);
static CArc_c_void ctx_arc_clone(CArc_c_void *self) {
    CArc_c_void ret = *self;
    ret.instance = self->clone_fn(self->instance);
    return ret;
}

void ctx_arc_drop(CArc_c_void *self) {
    if (self->drop_fn && self->instance) self->drop_fn(self->instance);
}
void cont_box_drop(CBox_c_void *self) {
    if (self->drop_fn && self->instance) self->drop_fn(self->instance);
}

static inline struct FeaturesGroup_CBox_c_void_____CArc_c_void borrow_features(void *self)  {
    struct FeaturesGroup_CBox_c_void_____CArc_c_void __ret = (((struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void *)self)->vtbl)->borrow_features(&((struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void *)self)->container);
    return __ret;
}

static inline struct FeaturesGroup_CBox_c_void_____CArc_c_void into_features(struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void self)  {
    CArc_c_void ___ctx = ctx_arc_clone(&self.container.context);
    struct FeaturesGroup_CBox_c_void_____CArc_c_void __ret = (self.vtbl)->into_features(self.container);
    ctx_arc_drop(&___ctx);
    return __ret;
}

static inline struct FeaturesGroup_____c_void__CArc_c_void * mut_features(void *self)  {
    struct FeaturesGroup_____c_void__CArc_c_void * __ret = (((struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void *)self)->vtbl)->mut_features(&((struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void *)self)->container);
    return __ret;
}

static inline void plugininner_drop(struct CGlueTraitObj_CBox_c_void_____PluginInnerVtbl_CGlueObjContainer_CBox_c_void_____CArc_c_void_____PluginInnerRetTmp_CArc_c_void______________CArc_c_void_____PluginInnerRetTmp_CArc_c_void self)  {
    cont_box_drop(&self.container.instance);
    ctx_arc_drop(&self.container.context);

}

static inline void featuresgroup_print_self(const void *self)  {
(((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_mainfeature)->print_self(&((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container);

}

static inline void featuresgroup_drop(struct FeaturesGroup_CBox_c_void_____CArc_c_void self)  {
    cont_box_drop(&self.container.instance);
    ctx_arc_drop(&self.container.context);

}

static inline struct FeaturesGroup_CBox_c_void_____CArc_c_void featuresgroup_clone(const void *self)  {
    struct FeaturesGroup_CBox_c_void_____CArc_c_void __ret;
    __ret.container = (((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_clone)->clone(&((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container);
    return __ret;
}

static inline void featuresgroup_dump_key_values(const void *self, KeyValueCallback callback)  {
(((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_keyvaluedumper)->dump_key_values(&((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container, callback);

}

static inline void featuresgroup_print_ints(const void *self, struct CIterator_i32 iter)  {
(((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_keyvaluedumper)->print_ints(&((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container, iter);

}

static inline void featuresgroup_write_key_value(void *self, struct CSliceRef_u8 name, uintptr_t val)  {
(((struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_keyvaluestore)->write_key_value(&((struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container, name, val);

}

static inline uintptr_t featuresgroup_get_key_value(const void *self, struct CSliceRef_u8 name)  {
    uintptr_t __ret = (((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->vtbl_keyvaluestore)->get_key_value(&((const struct FeaturesGroup_CBox_c_void_____CArc_c_void *)self)->container, name);
    return __ret;
}

static inline void featuresgroup_arc_mut_drop(struct FeaturesGroup_____c_void__CArc_c_void self)  {
    ctx_arc_drop(&self.container.context);

}

struct CollectBase {
    /* Pointer to array of data */
    char *buf;
    /* Capacity of the buffer (in elements) */
    size_t capacity;
    /* Current size of the buffer (in elements) */
    size_t size;
};

// For memcpy
#include <string.h>

static bool cb_collect_static_base(struct CollectBase *ctx, size_t elem_size, void *info) {

    if (ctx->size < ctx->capacity) {
        memcpy(ctx->buf + elem_size * ctx->size++, info, elem_size);
    }

    return ctx->size < ctx->capacity;
}

static bool cb_collect_dynamic_base(struct CollectBase *ctx, size_t elem_size, void *info) {

    if (!ctx->buf || ctx->size >= ctx->capacity) {
        size_t new_capacity = ctx->buf ? ctx->capacity * 2 : 64;
        char *buf = (char *)realloc(ctx->buf, elem_size * new_capacity);
        if (buf) {
            ctx->buf = buf;
            ctx->capacity = new_capacity;
        }
    }

    if (!ctx->buf || ctx->size >= ctx->capacity) return false;

    memcpy(ctx->buf + elem_size * ctx->size++, info, elem_size);

    return true;
}

struct BufferIterator {
    /* Pointer to the data buffer */
    const char *buf;
    /* Number of elements in the buffer */
    size_t size;
    /* Current element index */
    size_t i;
    /* Size of the data element */
    size_t sz_elem;
};

static bool buf_iter_next(struct BufferIterator *iter, void *out) {
    if (iter->i >= iter->size) return 1;
    memcpy(out, iter->buf + iter->i++ * iter->sz_elem, iter->sz_elem);
    return 0;
}

static inline bool cb_collect_static_KeyValue(struct CollectBase *ctx, KeyValue info) {
    return cb_collect_static_base(ctx, sizeof(KeyValue), &info);
}

static inline bool cb_collect_dynamic_KeyValue(struct CollectBase *ctx, KeyValue info) {
    return cb_collect_dynamic_base(ctx, sizeof(KeyValue), &info);
}

static inline bool cb_count_KeyValue(size_t *cnt, KeyValue info) {
    return ++(*cnt);
}


#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
