#include <cstdarg>
#include <cstring>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>


struct TypeLayout;

/** Destruct the object. */
template<typename T>
inline typename std::enable_if<!std::is_pointer<T>::value>::type mem_drop(T &&self) noexcept {
    std::move(self).drop();
}

template<typename T>
inline typename std::enable_if<std::is_pointer<T>::value>::type mem_drop(T &&self) noexcept {}

/** Forget the object's resources (null them out). */
template<typename T>
inline typename std::enable_if<!std::is_pointer<T>::value>::type mem_forget(T &self) noexcept {
    self.forget();
}

template<typename T>
inline typename std::enable_if<std::is_pointer<T>::value>::type mem_forget(T &self) noexcept {}

/** Defer mem_forget call when object goes out of scope. */
template<typename T>
struct DeferedForget {
    T &val;

    DeferedForget(T &val) : val(val) {}

    ~DeferedForget() {
        mem_forget(val);
    }
};

/** Workaround for void types in generic functions. */
struct StoreAll {
    constexpr auto operator[](StoreAll) const {
        return false;
    }

    template <class T>
    constexpr T && operator[](T &&t) const {
        return std::forward<T>(t);
    }

    template <class T>
    constexpr friend T && operator,(T &&t, StoreAll) {
        return std::forward<T>(t);
    }
};

template<typename CGlueCtx = void>
using CloneRetTmp = void;

template<typename CGlueCtx = void>
using KeyValueDumperRetTmp = void;

template<typename CGlueCtx = void>
using KeyValueStoreRetTmp = void;

template<typename CGlueCtx = void>
using MainFeatureRetTmp = void;

template<typename T = void>
using MaybeUninit = T;

template<typename T>
struct alignas(alignof(T)) RustMaybeUninit {
    char pad[sizeof(T)];
    constexpr T &assume_init() {
        return *(T *)this;
    }
    constexpr const T &assume_init() const {
        return *(const T *)this;
    }
};

/**
 * Wrapper around null-terminated C-style strings.
 *
 * Analog to Rust's `str`, [`ReprCStr`] borrows the underlying data.
 */
using ReprCStr = const char*;

/**
 * FFI-Safe Arc
 *
 * This is an FFI-Safe equivalent of Arc<T> and Option<Arc<T>>.
 */
template<typename T>
struct CArc {
    const T *instance;
    const T *(*clone_fn)(const T*);
    void (*drop_fn)(const T*);

    inline CArc clone() const noexcept {
        CArc ret;
        ret.instance = clone_fn(instance);
        ret.clone_fn = clone_fn;
        ret.drop_fn = drop_fn;
        return ret;
    }

    inline void drop() && noexcept {
        if (drop_fn)
            drop_fn(instance);
        forget();
    }

    inline void forget() noexcept {
        instance = nullptr;
        clone_fn = nullptr;
        drop_fn = nullptr;
    }
};

/**
 * FFI-safe box
 *
 * This box has a static self reference, alongside a custom drop function.
 *
 * The drop function can be called from anywhere, it will free on correct allocator internally.
 */
template<typename T>
struct CBox {
    T *instance;
    void (*drop_fn)(T*);

    CBox() = default;
    CBox(T *instance) : instance(instance), drop_fn(nullptr) {}
    CBox(T *instance, void (*drop_fn)(T *)) : instance(instance), drop_fn(drop_fn) {}
    template<typename U = T, class = typename std::enable_if<std::is_same<U, T>::value>::type, class = typename std::enable_if<!std::is_same<U, void>::value>::type>
    CBox(U &&instance) : instance(new U(instance)), drop_fn(&CBox::delete_fn) {}

    static void delete_fn(T *v) {
        delete v;
    }

    inline operator CBox<void> () const {
        CBox<void> ret;
        ret.instance = (void*)instance;
        ret.drop_fn = (void(*)(void *))drop_fn;
        return ret;
    }

    static inline CBox new_box() {
        CBox ret;
        ret.instance = new T;
        ret.drop_fn = &CBox::delete_fn;
        return ret;
    }

    inline void drop() && noexcept {
        if (drop_fn && instance)
            drop_fn(instance);
        forget();
    }

    inline void forget() noexcept {
        instance = nullptr;
        drop_fn = nullptr;
    }

    inline T *operator->() {
        return instance;
    }

    inline const T *operator->() const {
        return instance;
    }
};

template<typename CGlueInst = CBox<void>, typename CGlueCtx = CArc<void>>
struct FeaturesGroupContainer {
    typedef CGlueCtx Context;
    CGlueInst instance;
    CGlueCtx context;

    inline auto clone_context() noexcept {
        return context.clone();
    }

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
        mem_drop(std::move(context));
    }

    inline void forget() noexcept {
        mem_forget(instance);
        mem_forget(context);
    }
};

template<typename CGlueInst>
struct FeaturesGroupContainer<CGlueInst, void> {
    typedef void Context;
    CGlueInst instance;

    inline auto clone_context() noexcept {}

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
    }

    inline void forget() noexcept {
        mem_forget(instance);
    }
};

/**
 * CGlue vtable for trait MainFeature.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
template<typename CGlueC>
struct MainFeatureVtbl {
    typedef typename CGlueC::Context Context;
    void (*print_self)(const CGlueC *cont);
};

template<typename Impl>
struct MainFeatureVtblImpl : MainFeatureVtbl<typename Impl::Parent> {
constexpr MainFeatureVtblImpl() :
    MainFeatureVtbl<typename Impl::Parent> {
        &Impl::print_self
    } {}
};

/**
 * CGlue vtable for trait Clone.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
template<typename CGlueC>
struct CloneVtbl {
    typedef typename CGlueC::Context Context;
    CGlueC (*clone)(const CGlueC *cont);
};

template<typename Impl>
struct CloneVtblImpl : CloneVtbl<typename Impl::Parent> {
constexpr CloneVtblImpl() :
    CloneVtbl<typename Impl::Parent> {
        &Impl::clone
    } {}
};

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
template<typename T>
struct CSliceRef {
    const T *data;
    uintptr_t len;

    CSliceRef () = default;

    template<typename Cont, class = typename std::enable_if<
        std::is_same<decltype((*(const Cont *)nullptr).data()), const T *>::value
        && std::is_same<decltype((*(const Cont *)nullptr).size()), size_t>::value
    >::type>
    CSliceRef (const Cont &data) : data(data.data()), len(data.size()) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    CSliceRef (const char *value) : data((const T *)value), len(strlen(value)) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    CSliceRef (const char *value, uintptr_t len) : data((const T *)value), len(len) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    CSliceRef (const std::string &value) : data((const T *)value.data()), len(value.length()) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    inline operator std::string() const {
        return std::string((char *)data, len);
    }
};

struct KeyValue {
    CSliceRef<uint8_t> _0;
    uintptr_t _1;
};

template<typename T, typename F>
struct Callback {
    T *context;
    bool (*func)(T*, F);

    template<typename Container>
    static bool push_back(Container *context, F data) {
        context->push_back(data);
        return true;
    }

    template<typename Function>
    static bool functional(Function *function, F data) {
        return (*function)(data);
    }

    Callback() = default;

    template<typename OT, typename = decltype(std::declval<OT>().push_back(std::declval<F>()))>
    Callback(OT *cont) :
        context((T *)cont),
        func((decltype(func))(&Callback::push_back<OT>)) {}

    template<typename Function, typename = decltype(std::declval<Function>()(std::declval<F>()))>
    Callback(const Function &function) :
        context((T *)&function),
        func((decltype(func))(&Callback::functional<Function>)) {}

    constexpr operator Callback<void, F> &() {
        return *((Callback<void, F> *)this);
    }
};

template<typename T>
using OpaqueCallback = Callback<void, T>;

using KeyValueCallback = OpaqueCallback<KeyValue>;

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
template<typename T>
struct CIterator {
    void *iter;
    int32_t (*func)(void*, MaybeUninit<T> *out);

    class iterator : std::iterator<std::input_iterator_tag, T> {
        CIterator<T> *iter;
        RustMaybeUninit<T> data;
        bool initialized = false;
        bool end = false;

      public:
        explicit iterator() : end(true) {}

        explicit iterator(CIterator<T> *iter) : iter(iter) {
            end = iter->func(iter->iter, &data.assume_init());
        }

        iterator &operator++() {
            if (!iter || end) {
                return *this;
            }

            end = iter->func(iter->iter, &data.assume_init());

            return *this;
        }

        constexpr bool operator==(const iterator &other) const {
            return (end && other.end)
                || (!end && !other.end && data.assume_init() == other.data.assume_init());
        }

        constexpr bool operator!=(const iterator &other) const {
            return !(*this == other);
        }

        constexpr T &operator*() {
            return data.assume_init();
        }

        constexpr const T &operator*() const {
            return data.assume_init();
        }
    };

    constexpr iterator begin() {
        return iterator(this);
    }

    constexpr iterator end() {
        return iterator();
    }
};

template<typename Container>
struct CPPIterator {

    typedef typename Container::iterator::value_type T;

    CIterator<T> iter;
    typename Container::iterator cur, end;

    static int32_t next(void *data, MaybeUninit<T> *out) {
        CPPIterator *i = (CPPIterator *)data;

        if (i->cur == i->end) {
            return 1;
        } else {
            *out = *i->cur;
            i->cur++;
            return 0;
        }
    }

    CPPIterator(Container &cont)
        : cur(cont.begin()), end(cont.end())
    {
        iter.iter = &iter - offsetof(CPPIterator<Container>, iter);
        iter.func = &CPPIterator::next;
    }

    CPPIterator(CPPIterator &&o) {
        iter = o.iter;
        iter.iter = &this;
        cur = o.cur;
        end = o.end;
    }

    CPPIterator(CPPIterator &o) {
        iter = o.iter;
        iter.iter = &this;
        cur = o.cur;
        end = o.end;
    }

    inline operator CIterator<T> &() {
        return iter;
    }
};

/**
 * CGlue vtable for trait KeyValueDumper.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
template<typename CGlueC>
struct KeyValueDumperVtbl {
    typedef typename CGlueC::Context Context;
    void (*dump_key_values)(const CGlueC *cont, KeyValueCallback callback);
    void (*print_ints)(const CGlueC *cont, CIterator<int32_t> iter);
};

template<typename Impl>
struct KeyValueDumperVtblImpl : KeyValueDumperVtbl<typename Impl::Parent> {
constexpr KeyValueDumperVtblImpl() :
    KeyValueDumperVtbl<typename Impl::Parent> {
        &Impl::dump_key_values,
        &Impl::print_ints
    } {}
};

/**
 * CGlue vtable for trait KeyValueStore.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
template<typename CGlueC>
struct KeyValueStoreVtbl {
    typedef typename CGlueC::Context Context;
    void (*write_key_value)(CGlueC *cont, CSliceRef<uint8_t> name, uintptr_t val);
    uintptr_t (*get_key_value)(const CGlueC *cont, CSliceRef<uint8_t> name);
};

template<typename Impl>
struct KeyValueStoreVtblImpl : KeyValueStoreVtbl<typename Impl::Parent> {
constexpr KeyValueStoreVtblImpl() :
    KeyValueStoreVtbl<typename Impl::Parent> {
        &Impl::write_key_value,
        &Impl::get_key_value
    } {}
};

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
template<typename CGlueInst = CBox<void>, typename CGlueCtx = CArc<void>>
struct FeaturesGroup {
    const MainFeatureVtbl<FeaturesGroupContainer<CGlueInst, CGlueCtx>> *vtbl_mainfeature;
    const CloneVtbl<FeaturesGroupContainer<CGlueInst, CGlueCtx>> *vtbl_clone;
    const KeyValueDumperVtbl<FeaturesGroupContainer<CGlueInst, CGlueCtx>> *vtbl_keyvaluedumper;
    const KeyValueStoreVtbl<FeaturesGroupContainer<CGlueInst, CGlueCtx>> *vtbl_keyvaluestore;
    FeaturesGroupContainer<CGlueInst, CGlueCtx> container;

    FeaturesGroup() : container{} , vtbl_mainfeature{}, vtbl_clone{}, vtbl_keyvaluedumper{}, vtbl_keyvaluestore{} {}

    ~FeaturesGroup() noexcept {
        mem_drop(std::move(container));
    }

    typedef CGlueCtx Context;

    inline auto print_self() const noexcept {
    (this->vtbl_mainfeature)->print_self(&this->container);

    }

    inline FeaturesGroup clone() const noexcept {
        FeaturesGroup __ret;
            __ret.vtbl_mainfeature = this->vtbl_mainfeature;
            __ret.vtbl_clone = this->vtbl_clone;
            __ret.vtbl_keyvaluedumper = this->vtbl_keyvaluedumper;
            __ret.vtbl_keyvaluestore = this->vtbl_keyvaluestore;
        __ret.container = (this->vtbl_clone)->clone(&this->container);
        return __ret;
    }

    inline auto dump_key_values(KeyValueCallback callback) const noexcept {
    (this->vtbl_keyvaluedumper)->dump_key_values(&this->container, callback);

    }

    inline auto print_ints(CIterator<int32_t> iter) const noexcept {
    (this->vtbl_keyvaluedumper)->print_ints(&this->container, iter);

    }

    inline auto write_key_value(CSliceRef<uint8_t> name, uintptr_t val) noexcept {
    (this->vtbl_keyvaluestore)->write_key_value(&this->container, name, val);

    }

    inline auto get_key_value(CSliceRef<uint8_t> name) const noexcept {
        uintptr_t __ret = (this->vtbl_keyvaluestore)->get_key_value(&this->container, name);
        return __ret;
    }

};

/**
 * Temporary return value structure, for returning wrapped references.
 *
 * This structure contains data for each vtable function that returns a reference to
 * an associated type. Note that these temporary values should not be accessed
 * directly. Use the trait functions.
 */
template<typename CGlueCtx = CArc<void>>
struct PluginInnerRetTmp {
    MaybeUninit<FeaturesGroup<void*, CGlueCtx>> mut_features;
};

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
template<typename T, typename C, typename R>
struct CGlueObjContainer {
    typedef C Context;
    T instance;
    C context;
    RustMaybeUninit<R> ret_tmp;

    inline auto clone_context() noexcept {
        return context.clone();
    }

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
        mem_drop(std::move(context));
    }

    inline void forget() noexcept {
        mem_forget(instance);
        mem_forget(context);
    }
};

template<typename T, typename R>
struct CGlueObjContainer<T, void, R> {
    typedef void Context;
    T instance;
    RustMaybeUninit<R> ret_tmp;

    inline auto clone_context() noexcept {}

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
    }

    inline void forget() noexcept {
        mem_forget(instance);
    }
};

template<typename T, typename C>
struct CGlueObjContainer<T, C, void> {
    typedef C Context;
    T instance;
    C context;

    inline auto clone_context() noexcept {
        return context.clone();
    }

    void drop() && noexcept {
        mem_drop(std::move(instance));
        mem_drop(std::move(context));
    }

    void forget() noexcept {
        mem_forget(instance);
        mem_forget(context);
    }
};

template<typename T>
struct CGlueObjContainer<T, void, void> {
    typedef void Context;
    T instance;

    auto clone_context() noexcept {}

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
    }

    inline void forget() noexcept {
        mem_forget(instance);
    }
};

/**
 * CGlue vtable for trait PluginInner.
 *
 * This virtual function table contains ABI-safe interface for the given trait.
 */
template<typename CGlueC>
struct PluginInnerVtbl {
    typedef typename CGlueC::Context Context;
    FeaturesGroup<CBox<void>, Context> (*borrow_features)(CGlueC *cont);
    FeaturesGroup<CBox<void>, Context> (*into_features)(CGlueC cont);
    FeaturesGroup<void*, Context> *(*mut_features)(CGlueC *cont);
};

template<typename Impl>
struct PluginInnerVtblImpl : PluginInnerVtbl<typename Impl::Parent> {
constexpr PluginInnerVtblImpl() :
    PluginInnerVtbl<typename Impl::Parent> {
        &Impl::borrow_features,
        &Impl::into_features,
        &Impl::mut_features
    } {}
};

/**
 * Simple CGlue trait object.
 *
 * This is the simplest form of CGlue object, represented by a container and vtable for a single
 * trait.
 *
 * Container merely is a this pointer with some optional temporary return reference context.
 */
template<typename T, typename V, typename C, typename R>
struct CGlueTraitObj {
    const V *vtbl;
    CGlueObjContainer<T, C, R> container;
};

/**
 * Base CGlue trait object for trait PluginInner.
 */
template<typename CGlueInst = CBox<void>, typename CGlueCtx = CArc<void>>
using PluginInnerBase = CGlueTraitObj<CGlueInst, PluginInnerVtbl<CGlueObjContainer<CGlueInst, CGlueCtx, PluginInnerRetTmp<CGlueCtx>>>, CGlueCtx, PluginInnerRetTmp<CGlueCtx>>;

/**
 * CtxBoxed CGlue trait object for trait PluginInner with context.
 */
template<typename CGlueT, typename CGlueCtx = CArc<void>>
using PluginInnerBaseCtxBox = PluginInnerBase<CBox<CGlueT>, CGlueCtx>;

/**
 * Boxed CGlue trait object for trait PluginInner with a [`CArc`](cglue::arc::CArc) reference counted context.
 */
template<typename CGlueT, typename CGlueC>
using PluginInnerBaseArcBox = PluginInnerBaseCtxBox<CGlueT, CArc<CGlueC>>;

/**
 * Opaque Boxed CGlue trait object for trait PluginInner with a [`CArc`](cglue::arc::CArc) reference counted context.
 */
using PluginInnerArcBox = PluginInnerBaseArcBox<void, void>;
// Typedef for default contaienr and context type
using PluginInner = PluginInnerArcBox;

extern "C" {

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
int32_t load_plugin(ReprCStr name, MaybeUninit<PluginInnerArcBox> *ok_out);

} // extern "C"

template<typename T, typename C, typename R>
struct CGlueTraitObj<T, MainFeatureVtbl<CGlueObjContainer<T, C, R>>, C, R> {
    const MainFeatureVtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{} {}

    ~CGlueTraitObj() noexcept {
        mem_drop(std::move(container));
    }

    typedef C Context;

    inline auto print_self() const noexcept {
    (this->vtbl)->print_self(&this->container);

    }

};

template<typename T, typename C, typename R>
struct CGlueTraitObj<T, CloneVtbl<CGlueObjContainer<T, C, R>>, C, R> {
    const CloneVtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{} {}

    ~CGlueTraitObj() noexcept {
        mem_drop(std::move(container));
    }

    typedef C Context;

    inline CGlueTraitObj clone() const noexcept {
        CGlueTraitObj __ret;
            __ret.vtbl = this->vtbl;
        __ret.container = (this->vtbl)->clone(&this->container);
        return __ret;
    }

};

template<typename T, typename C, typename R>
struct CGlueTraitObj<T, KeyValueDumperVtbl<CGlueObjContainer<T, C, R>>, C, R> {
    const KeyValueDumperVtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{} {}

    ~CGlueTraitObj() noexcept {
        mem_drop(std::move(container));
    }

    typedef C Context;

    inline auto dump_key_values(KeyValueCallback callback) const noexcept {
    (this->vtbl)->dump_key_values(&this->container, callback);

    }

    inline auto print_ints(CIterator<int32_t> iter) const noexcept {
    (this->vtbl)->print_ints(&this->container, iter);

    }

};

template<typename T, typename C, typename R>
struct CGlueTraitObj<T, KeyValueStoreVtbl<CGlueObjContainer<T, C, R>>, C, R> {
    const KeyValueStoreVtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{} {}

    ~CGlueTraitObj() noexcept {
        mem_drop(std::move(container));
    }

    typedef C Context;

    inline auto write_key_value(CSliceRef<uint8_t> name, uintptr_t val) noexcept {
    (this->vtbl)->write_key_value(&this->container, name, val);

    }

    inline auto get_key_value(CSliceRef<uint8_t> name) const noexcept {
        uintptr_t __ret = (this->vtbl)->get_key_value(&this->container, name);
        return __ret;
    }

};

template<typename T, typename C, typename R>
struct CGlueTraitObj<T, PluginInnerVtbl<CGlueObjContainer<T, C, R>>, C, R> {
    const PluginInnerVtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{} {}

    ~CGlueTraitObj() noexcept {
        mem_drop(std::move(container));
    }

    typedef C Context;

    inline auto borrow_features() noexcept {
        FeaturesGroup<CBox<void>, Context> __ret = (this->vtbl)->borrow_features(&this->container);
        return __ret;
    }

    inline auto into_features() && noexcept {
        auto ___ctx = StoreAll()[this->container.clone_context(), StoreAll()];
        FeaturesGroup<CBox<void>, Context> __ret = (this->vtbl)->into_features(this->container);
        mem_forget(this->container);
        return __ret;
    }

    inline auto mut_features() noexcept {
        FeaturesGroup<void*, Context> * __ret = (this->vtbl)->mut_features(&this->container);
        return __ret;
    }

};

