//! # C++ code generator.
//!
//! This generator performs multiple things:
//!
//! ## Cleanup the types to work properly.
//!
//! Add typedefs where needed, add struct specializations for when the type param is void, etc.
//!
//! ## Generate trait function wrappers.
//!
//! For every group, and a single-trait cglue objects, all functions have been wrapped to call into
//! the vtable. In groups, if 2 traits have the same function name, all such functions will be
//! prefixed with each trait's name.
//!
//! ## Generate clone/drop functions.
//!
//! Copy constructor is not the way to clone objects. Use `object.clone()` function.
//!
//! Groups and CGlueTraitObj types have destructors defined. **Due to this, these types are not
//! fully compatible with C ABI!** Specifically, you are not able to pass objects of this type
//! by-value, because C++ will implicitly pass them by-reference.
//!
//! ## Simplify passing stl containers as iterators.
//!
//! Special `CPPIterator` type is available that takes in any container that has `begin` and `end`
//! functions. It casts itself to `CIterator` implicitly when needed.
//!
//! With C++17 you do not need to specify the CPPIterator's type, but sadly, in older standards you
//! may still have to.
//!
//! ## Implement `input_iterator` for `CIterator`.
//!
//! ## Add conversions between `CSlice`s, `char *`, and `std::string` types.
//!
//! ## Allow `Callback`s to be built with containers and lambdas.
//!
//! Containers must be passed by pointer, while lambdas are taken in as const references.
//!
//! ## Create vtable impl types.
//!
//! Basically, this is a quick way to build a vtable for an object, if you have a struct defined in
//! a specific manner.
//!
//! Given a `MainFeature` trait, with vtable:
//!
//! ```ignore
//! template<typename CGlueC>
//! struct MainFeatureVtbl {
//!     void (*print_self)(const CGlueC *cont);
//! };
//! ```
//!
//! The following builder struct will be created:
//!
//! ```ignore
//! template<typename Impl>
//! struct MainFeatureVtblImpl : MainFeatureVtbl<typename Impl::Parent> {
//! constexpr MainFeatureVtblImpl() :
//!     MainFeatureVtbl<typename Impl::Parent> {
//!         &Impl::print_self
//!     } {}
//! };
//! ```
//!
//! To allow building a vtable, provide an implementation something like this:
//!
//! ```ignore
//! template<typename T = CBox<KvStore>, typename C = CArc<void>>
//! struct KvStoreContainer : CGlueObjContainer<T, C, MainFeatureRetTmp<C>> {
//!
//!     using Parent = CGlueObjContainer<T, C, MainFeatureRetTmp<C>>;
//!
//!     static void print_self(const Parent *self) {
//!         for (const auto &e : self->instance->map) {
//!             printf("%s: %zu\n", e.first.c_str(), e.second);
//!         }
//!     }
//! };
//! ```
//!
//! Where `KvStore` can be a structure that contains underlying data to print (the `map` variable).
//!
//! Then, you just need to define the vtable in the global state, or wherever they are needed:
//!
//! ```ignore
//! constexpr MainFeatureVtblImpl<KvStoreContainer<>> main_feature_vtbl;
//! constexpr MainFeatureVtblImpl<KvStoreContainer<KvStore *>> main_feature_mut_vtbl;
//! ```
//!
//! You may still have to cast the address of the vtable to erase the type information, and it is
//! recommended to do that at the very last step. However, this is error prone, and in the future
//! type erasure on C++ side may be done automatically.
//!

use crate::config::*;
use crate::types::*;
use itertools::Itertools;
use log::trace;
use regex::*;
use std::collections::HashMap;

pub fn is_cpp(header: &str) -> Result<bool> {
    Ok(Regex::new(
        r"
template<typename|
using [^\s]+ =|
#include <cstd[^>]+>",
    )?
    .is_match(header))
}

pub fn parse_header(header: &str, config: &Config) -> Result<String> {
    // PREPROCESSING:

    // Fix up the MaybeUninit
    // Basically, we strip it completely, and then define `RustMaybeUninit` for special cases.

    let header = &strip_maybe_uninit(header);

    // COLLECTION:

    // Collect zsized ret tmps
    let zsr_regex = zero_sized_ret_regex()?;
    let zst_rets = zsr_regex
        .captures_iter(header)
        .map(|c| c["trait"].to_string())
        .collect::<Vec<_>>();

    for cap in &zst_rets {
        trace!("CAP: {}", cap);
    }

    // Collect all vtables
    let vtbl_regex = vtbl_regex()?;
    let vtbls = vtbl_regex
        .captures_iter(header)
        .filter(|c| c["trait"] == c["trait2"])
        .map(|c| Vtable::new(c["trait"].to_string(), &c["functions"], "CGlueC"))
        .collect::<Result<Vec<_>>>()?;

    let mut vtbls_map = HashMap::new();

    for vtbl in &vtbls {
        vtbls_map.insert(vtbl.name.as_str(), vtbl);
        trace!("TRAIT: {}", vtbl.name);
    }

    // Collect groups
    let groups_regex = groups_regex(&vtbls, None)?;
    let groups = groups_regex
        .captures_iter(header)
        .map(|c| Group::new(c["group"].to_string(), &c["vtbls"]))
        .collect::<Result<Vec<_>>>()?;

    for g in &groups {
        trace!("GROUP: {} {:?}", g.name, g.vtables);
    }

    // Check if we need to typedef `TypeLayout`
    let type_layout_re =
        Regex::new(r"((((typedef [\s]+)|struct) TypeLayout)|(using TypeLayout =))")?;
    let needs_type_layout =
        header.contains("const TypeLayout *") && !type_layout_re.is_match(header);

    // PROCESSING:

    let header = header.replace(
        "struct MaybeUninit;",
        r"struct alignas(alignof(T)) RustMaybeUninit {
    char pad[sizeof(T)];
    inline T &assume_init() {
        return *(T *)this;
    }
    constexpr const T &assume_init() const {
        return *(const T *)this;
    }
};",
    );

    // Add string conversion to CSlices
    let header = Regex::new(
        r"(?P<definition>template<typename T>
struct (?P<class>CSlice(Ref|Mut)) \{
    (?P<constness>(const )?)T \*data;
    uintptr_t len;)
\};"
    )?
.replace_all(
        &header,
    r"$definition

    $class () = default;

    template<typename Cont, class = typename std::enable_if<
        std::is_same<decltype((*(${constness}Cont *)nullptr).data()), ${constness}T *>::value
        && std::is_same<decltype((*(${constness}Cont *)nullptr).size()), size_t>::value
    >::type>
    $class (${constness}Cont &data) : data(data.data()), len(data.size()) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    $class (${constness}char *value) : data((${constness}T *)value), len(strlen(value)) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    $class (${constness}char *value, uintptr_t len) : data((${constness}T *)value), len(len) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    $class (${constness}std::string &value) : data((${constness}T *)value.data()), len(value.length()) {}

    template<typename U = T, class = typename std::enable_if<
        (std::is_same<T, char>::value || std::is_same<T, unsigned char>::value)
        && std::is_same<T, U>::value
    >::type>
    inline operator std::string() const {
        return std::string((char *)data, len);
    }
};"
    );

    // Add cstring include if it wasn't included
    let header = if header.contains("#include <cstring>") {
        header
    } else {
        Regex::new("#include <c[^>]+>")?.replace(&header, "$0\n#include <cstring>")
    };

    // Bridge common stl containers to callbacks and iterators
    let header = Regex::new(
        r"(?P<definition_start>template<typename T>
struct CIterator \{
    void \*iter;
    int32_t \(\*func\)\(void\*, T \*out\);)
\};",
    )?
    .replace(
        &header,
        r"$definition_start

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

        inline T &operator*() {
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

    static int32_t next(void *data, T *out) {
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
};",
    );

    let header = Regex::new(
        r"(?P<definition>template<typename T, typename F>
struct Callback \{
    T \*context;
    bool \(\*func\)\(T\*, F\);)
\};",
    )?
    .replace(
        &header,
        r"$definition

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
};",
    );

    let header_regex = Regex::new(
        r"(?P<start>(/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
)?template<typename)",
    )?;

    // Add mem_drop and mem_forget methods
    let header = header_regex.replace(
        &header,
        r"
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
    constexpr bool operator[](StoreAll) const {
        return false;
    }

    template <class T>
    constexpr T && operator[](T &&t) const {
        return std::forward<T>(t);
    }

    template <class T>
    friend T && operator,(T &&t, StoreAll) {
        return std::forward<T>(t);
    }
};

$start",
    );

    // Add TypeLayout forward decl if needed
    let header = if needs_type_layout {
        header_regex.replace(
            &header,
            r"struct TypeLayout;

$start",
        )
    } else {
        header
    };

    // Add CBox drop and methods
    let header = Regex::new(
        r"(?P<definition>template<typename T>
struct CBox \{
    T \*instance;
    void \(\*drop_fn\)\(T\*\);)
\};",
    )?
    .replace(
        &header,
        r"${definition}

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
};",
    );

    // Add CArc clone and drop methods
    let header = Regex::new(
        r"(?P<definition>template<typename T>
struct CArc \{
    const T \*instance;
    const T \*\(\*clone_fn\)\(const T\*\);
    void \(\*drop_fn\)\(const T\*\);)
\};",
    )?
    .replace(
        &header,
        r"${definition}

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
};",
    );

    // Remove zsized ret tmps
    let header = zsr_regex.replace_all(
        &header,
        r"
template<typename CGlueCtx = void>
using ${trait}RetTmp = void;
",
    );

    let gr_regex = group_ret_tmp_regex(&zst_rets)?;
    let header = gr_regex.replace_all(&header, "");

    // Add `typedef typename CGlueC::Context Context;` to each vtable
    // Also add vtable builder to each vtable
    let header = vtbl_regex.replace_all(&header, |caps: &Captures| {
        let decl = &caps["declaration"];
        let funcs = &caps["functions"];
        let tr = &caps["trait"];

        let mut impl_definitions = String::new();

        for (i, v) in vtbls_map[&tr].functions.iter().enumerate() {
            if i > 0 {
                impl_definitions += ",\n        ";
            }
            impl_definitions += "&Impl::";
            impl_definitions += &v.name;
        }

        format!(
            r"{decl} {{
    typedef typename CGlueC::Context Context;
    {funcs}
}};

template<typename Impl>
struct {tr}VtblImpl : {tr}Vtbl<typename Impl::Parent> {{
constexpr {tr}VtblImpl() :
    {tr}Vtbl<typename Impl::Parent> {{
        {impl_definitions}
    }} {{}}
}};",
            decl = decl,
            funcs = funcs,
            tr = tr,
            impl_definitions = impl_definitions
        )
    });

    // Add Context typedef to CGlueObjContainer
    // Create CGlueObjContainer type specializations
    // Add drop and forget methods to it.

    let header = obj_container_regex()?.replace_all(
        &header,
        r"$declaration {
    typedef C Context;
    $fields
    RustMaybeUninit<R> ret_tmp;

    inline Context clone_context() noexcept {
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

    inline Context clone_context() noexcept {}

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

    inline Context clone_context() noexcept {
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

    inline Context clone_context() noexcept {}

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
    }

    inline void forget() noexcept {
        mem_forget(instance);
    }
};",
    );

    // Add Context typedef to group containers
    // Create group container specializations

    let gr_regex = Regex::new("\\s(?P<ret_tmp>([^\\s])RetTmp<CGlueCtx>)")?;

    let header = group_container_regex(&groups)?.replace_all(&header, |caps: &Captures| {
        let ret_tmps = gr_regex.replace_all(&caps["ret_tmps"], "RustMaybeUninit<$ret_tmp>");

        format!(
            r"{declaration} {{
    typedef CGlueCtx Context;
    {fields}{ret_tmps}

    inline Context clone_context() noexcept {{
        return context.clone();
    }}

    inline void drop() && noexcept {{
        mem_drop(std::move(instance));
        mem_drop(std::move(context));
    }}

    inline void forget() noexcept {{
        mem_forget(instance);
        mem_forget(context);
    }}
}};

template<typename CGlueInst>
struct {group}Container<CGlueInst, void> {{
    typedef void Context;
    CGlueInst instance;

    inline Context clone_context() noexcept {{}}

    inline void drop() && noexcept {{
        mem_drop(std::move(instance));
    }}

    inline void forget() noexcept {{
        mem_forget(instance);
    }}
}};",
            declaration = &caps["declaration"],
            fields = &caps["fields"],
            group = &caps["group"],
            ret_tmps = ret_tmps
        )
    });

    let mut header = header.to_string();

    // Create vtable functions to group objects
    for g in groups {
        let helpers = g.create_wrappers(&vtbls_map, "container");

        let mut init_vtbls = String::new();

        for (_, v) in &g.vtables {
            init_vtbls += &format!(", {}{{}}", v);
        }

        header = self::groups_regex(&vtbls, Some(g.name.as_str()))?
            .replace_all(
                &header,
                &format!(
                    r"$definition_start

    {name}() : container{{}} {init_vtbls} {{}}

    ~{name}() noexcept {{
        mem_drop(std::move(container));
    }}

    typedef CGlueCtx Context;
{helpers}
}};",
                    name = g.name,
                    helpers = helpers,
                    init_vtbls = init_vtbls
                ),
            )
            .to_string();
    }

    // Create CGlueTraitObj vtable functions
    let mut trait_obj_specs = String::new();

    for v in &vtbls {
        trait_obj_specs.push_str(&format!(
            r"
template<typename T, typename C, typename R>
struct CGlueTraitObj<T, {vtbl}Vtbl<CGlueObjContainer<T, C, R>>, C, R> {{
    const {vtbl}Vtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    CGlueTraitObj() : container{{}} {{}}

    ~CGlueTraitObj() noexcept {{
        mem_drop(std::move(container));
    }}

    typedef C Context;
{wrappers}
}};
",
            vtbl = v.name,
            wrappers = v.create_wrappers(
                ("container", "vtbl"),
                |_| false,
                ("CGlueTraitObj", &["vtbl"])
            )
        ));
    }

    trait_obj_specs.push_str("\n$0");

    // Probably a more hacky version - put specializations at the end of the file,
    // so that we do not encounter incomplete types.
    let mut header = Regex::new(r"(#endif|$)")?
        .replace(&header, trait_obj_specs)
        .to_string();

    // Create shortened typedefs for all the types (if configured)
    if config.default_container.is_some() || config.default_context.is_some() {
        let container_map = ContainerType::get_prefix_map();
        let context_map = ContextType::get_prefix_map();

        let default_cont = config.default_container.as_deref().unwrap_or("");
        let default_ctx = config.default_context.as_deref().unwrap_or("");

        let mut ty = String::new();

        // Groups merely need default types (TODO: Handle extra generics somehow?)

        if let Some(ctx) = context_map.get(default_ctx) {
            let ctx_regex = Regex::new(
                r"typename CGlueCtx(?P<eq> =)?(?P<specialization>.*>
struct [^\{\}\n]+<.*CGlueCtx.*>)?",
            )?;

            header = ctx_regex
                .replace_all(&header, |caps: &Captures| {
                    if caps.name("eq").is_some() || caps.name("specialization").is_some() {
                        caps.get(0).unwrap().as_str().to_string()
                    } else {
                        format!("typename CGlueCtx = {}", ctx.cpp_type)
                    }
                })
                .to_string();

            ty += ctx.ty_prefix;
        }

        if let Some(ctx) = container_map.get(default_cont) {
            let ctx_regex = Regex::new(
                r"typename CGlueInst(?P<eq> =)?(?P<specialization>.*>
struct [^\{\}\n]+<.*CGlueInst.*>)?",
            )?;

            header = ctx_regex
                .replace_all(&header, |caps: &Captures| {
                    if caps.name("eq").is_some() || caps.name("specialization").is_some() {
                        caps.get(0).unwrap().as_str().to_string()
                    } else {
                        format!("typename CGlueInst = {}", ctx.cpp_type)
                    }
                })
                .to_string();

            ty += ctx.ty_prefix;
        }

        if !ty.is_empty() {
            let using_regex = Regex::new(&format!(
                r"(?P<template>template<(?P<template_args>.*)>
)?using (?P<type>.+){} = [^;]+;",
                ty
            ))?;

            header = using_regex
                .replace_all(&header, |caps: &Captures| {
                    let mut ret = caps.get(0).unwrap().as_str().to_string();

                    let base_ty = &caps["type"];

                    if !header.contains(&format!("using {} =", base_ty))
                        && !header.contains(&format!("struct {} {{", base_ty))
                    {
                        ret += "\n// Typedef for default contaienr and context type\n";

                        if let Some(tmplt) = caps.name("template") {
                            ret += tmplt.as_str();
                        }

                        ret += &format!(
                            "using {base_ty} = {base_ty}{ctxcont}",
                            base_ty = base_ty,
                            ctxcont = ty
                        );

                        if let Some(args) = caps.name("template_args") {
                            ret += "<";
                            for arg in args.as_str().split("typename").map(|s| s.trim()) {
                                ret += arg;
                            }
                            ret += ">";
                        }

                        ret += ";";
                    }

                    ret
                })
                .into();
        }
    }

    Ok(header)
}

fn strip_maybe_uninit(header: &str) -> String {
    let mut out = String::new();

    let mut iter = header.split("MaybeUninit<");

    if let Some(v) = iter.next() {
        out += v;
    }

    for v in iter {
        let mut cnt = 1;
        let mut done = false;

        for c in v.chars() {
            if !done {
                if c == '<' {
                    cnt += 1;
                } else if c == '>' {
                    cnt -= 1;
                    if cnt == 0 {
                        done = true;
                        continue;
                    }
                }
            }

            out.push(c);
        }
    }

    out
}

fn zero_sized_ret_regex() -> Result<Regex> {
    Regex::new(
        r"
/\*\*
 \* Type definition for temporary return value wrapping storage.
 \*
 \* The trait does not use return wrapping, thus is a typedef to `PhantomData`.
 \*
 \* Note that `cbindgen` will generate wrong structures for this type. It is important
 \* to go inside the generated headers and fix it - all RetTmp structures without a
 \* body should be completely deleted, both as types, and as fields in the
 \* groups/objects. If C\+\+11 templates are generated, it is important to define a
 \* custom type for CGlueTraitObj that does not have `ret_tmp` defined, and change all
 \* type aliases of this trait to use that particular structure.
 \*/
template<typename CGlueCtx = void>
struct (?P<trait>\w+)RetTmp;
",
    )
    .map_err(Into::into)
}

fn group_ret_tmp_regex(zero_sized: &[String]) -> Result<Regex> {
    let typenames = zero_sized.join("|");
    let typenames_lc = zero_sized
        .iter()
        .map(String::as_str)
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(
        "\\s*({})RetTmp<CGlueCtx> ret_tmp_({});",
        typenames, typenames_lc
    ))
    .map_err(Into::into)
}

fn vtbl_regex() -> Result<Regex> {
    Regex::new(
        r"(?P<declaration>/\*\*
 \* CGlue vtable for trait (?P<trait2>\w+).
 \*
 \* This virtual function table contains ABI-safe interface for the given trait.
 \*/
template<typename CGlueC>
struct (?P<trait>\w+)Vtbl) \{
    (?P<functions>[^\}]+)
\};",
    )
    .map_err(Into::into)
}

fn groups_regex(vtbls: &[Vtable], explicit_group: Option<&str>) -> Result<Regex> {
    let group_fmt = explicit_group.unwrap_or("\\w+");

    let vtbl_names =
        Itertools::intersperse(vtbls.iter().map(|v| v.name.as_str()), "|").collect::<String>();

    Regex::new(
        &format!(r"(?P<definition_start> \* `as_ref_`, and `as_mut_` functions obtain references to safe objects, but do not
 \* perform any memory transformations either. They are the safest to use, because
 \* there is no risk of accidentally consuming the whole object.
 \*/
template<typename CGlueInst, typename CGlueCtx>
struct (?P<group>{}) \{{
    (?P<vtbls>(\s*const ({})Vtbl<.*> \*vtbl_\w+;)*)
    (?P<group2>\w+)Container<CGlueInst, CGlueCtx> container;)
\}};", group_fmt, vtbl_names),
    )
    .map_err(Into::into)
}

fn obj_container_regex() -> Result<Regex> {
    Regex::new(
        r"(?P<declaration>template<typename T, typename C, typename R>
struct CGlueObjContainer) \{
    (?P<fields>T instance;
    C context;)
    R ret_tmp;
\};",
    )
    .map_err(Into::into)
}

fn group_container_regex(groups: &[Group]) -> Result<Regex> {
    let typenames =
        Itertools::intersperse(groups.iter().map(|g| g.name.as_str()), "|").collect::<String>();
    let typenames_lc = groups
        .iter()
        .map(|g| g.name.to_lowercase())
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(
        r"(?P<declaration>template<typename CGlueInst, typename CGlueCtx>
struct (?P<group>{})Container) \{{
    (?P<fields>CGlueInst instance;
    CGlueCtx context;)(?P<ret_tmps>(
    ({})RetTmp ret_tmp_{};)*)
\}};",
        typenames, typenames, typenames_lc
    ))
    .map_err(Into::into)
}
