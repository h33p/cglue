use crate::types::*;
use itertools::Itertools;
use regex::*;
use std::collections::HashMap;

pub fn parse_header(header: &str) -> Result<String> {
    // COLLECTION:

    // Collect zsized ret tmps
    let zsr_regex = zero_sized_ret_regex()?;
    let zst_rets = zsr_regex
        .captures_iter(header)
        .map(|c| c["trait"].to_string())
        .collect::<Vec<_>>();

    for cap in &zst_rets {
        println!("CAP: {}", cap);
    }

    // Collect all vtables
    let vtbl_regex = vtbl_regex()?;
    let vtbls = vtbl_regex
        .captures_iter(header)
        .filter(|c| c["trait"] == c["trait2"])
        .map(|c| Vtable::new(c["trait"].to_string(), &c["functions"]))
        .collect::<Result<Vec<_>>>()?;

    let mut vtbls_map = HashMap::new();

    for vtbl in &vtbls {
        vtbls_map.insert(vtbl.name.as_str(), vtbl);
        println!("TRAIT: {}", vtbl.name);
    }

    // Collect groups
    let groups_regex = groups_regex(&vtbls, None)?;
    let groups = groups_regex
        .captures_iter(header)
        .map(|c| Group::new(c["group"].to_string(), &c["vtbls"]))
        .collect::<Result<Vec<_>>>()?;

    for g in &groups {
        println!("GROUP: {} {:?}", g.name, g.vtables);
    }

    // PROCESSING:

    // Fix up the MaybeUninit
    let header = maybe_uninit_regex()?.replace_all(
        header,
        r"struct alignas(alignof(T)) MaybeUninit {
    char pad[sizeof(T)];

    constexpr T &assume_init() {
        return *(T *)this;
    }

    constexpr const T &assume_init() const {
        return *(const T *)this;
    }
};",
    );

    // Add mem_drop and mem_forget methods
    let header = Regex::new(
        r"(?P<start>(/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
)?template<typename)",
    )?
    .replace(
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

$start",
    );

    // Add CBox drop methods
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

    inline void drop() && noexcept {
        if (drop_fn && instance)
            drop_fn(instance);
        forget();
    }

    inline void forget() noexcept {
        instance = nullptr;
        drop_fn = nullptr;
    }
};",
    );

    // Add COptArc clone and drop methods
    let header = Regex::new(
        r"(?P<definition>template<typename T>
struct COptArc \{
    const T \*instance;
    const T \*\(\*clone_fn\)\(const T\*\);
    void \(\*drop_fn\)\(const T\*\);)
\};",
    )?
    .replace(
        &header,
        r"${definition}

    inline COptArc clone() const noexcept {
        COptArc ret;
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
    let header = zsr_regex.replace_all(&header, "");

    let gr_regex = group_ret_tmp_regex(&zst_rets)?;
    let header = gr_regex.replace_all(&header, "");

    // Add `typedef typename CGlueC::Context Context;` to each vtable
    let header = vtbl_regex.replace_all(
        &header,
        r"$declaration {
    typedef typename CGlueC::Context Context;
    $functions
};",
    );

    // Add Context typedef to CGlueObjContainer
    // Create CGlueObjContainer type specializations
    // Add drop and forget methods to it.

    let header = obj_container_regex()?.replace_all(
        &header,
        r"$declaration {
    typedef C Context;
    $fields

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
    R ret_tmp;

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
};",
    );

    // Add Context typedef to group containers
    // Create group container specializations

    let header = group_container_regex(&groups)?.replace_all(
        &header,
        r"$declaration {
    typedef CGlueCtx Context;
    $fields

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
struct ${group}Container<CGlueInst, void> {
    typedef void Context;
    CGlueInst instance;

    inline auto clone_context() noexcept {}

    inline void drop() && noexcept {
        mem_drop(std::move(instance));
    }

    inline void forget() noexcept {
        mem_forget(instance);
    }
};",
    );

    let mut header = header.to_string();

    // Create vtable functions to group objects
    for g in groups {
        let helpers = g.create_wrappers(&vtbls_map, "(this->container)");
        header = self::groups_regex(&vtbls, Some(g.name.as_str()))?
            .replace_all(
                &header,
                &format!(
                    r"$definition_start

    ~{}() noexcept {{
        mem_drop(std::move(container));
    }}
{}
}};",
                    g.name, helpers
                ),
            )
            .to_string();
    }

    // Create CGlueTraitObj vtable functions
    let mut trait_obj_specs = "$0\n".to_string();

    for v in &vtbls {
        trait_obj_specs.push_str(&format!(
            r"
template<typename T, typename C, typename R>
struct CGlueTraitObj<T, {vtbl}Vtbl<CGlueObjContainer<T, C, R>>, C, R> {{
    const {vtbl}Vtbl<CGlueObjContainer<T, C, R>> *vtbl;
    CGlueObjContainer<T, C, R> container;

    ~CGlueTraitObj() noexcept {{
        mem_drop(std::move(container));
    }}
{wrappers}
}};
",
            vtbl = v.name,
            wrappers = v.create_wrappers(
                "(this->container)",
                "this->vtbl",
                "CGlueTraitObj",
                &["vtbl"]
            )
        ));
    }

    let header = trait_obj_regex()?.replace_all(&header, trait_obj_specs);

    Ok(header.into())
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

    let vtbl_names = vtbls
        .iter()
        .map(|v| v.name.as_str())
        .intersperse("|")
        .collect::<String>();

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
    C context;
    R ret_tmp;)
\};",
    )
    .map_err(Into::into)
}

fn group_container_regex(groups: &[Group]) -> Result<Regex> {
    let typenames = groups
        .iter()
        .map(|g| g.name.as_str())
        .intersperse("|")
        .collect::<String>();
    Regex::new(&format!(
        r"(?P<declaration>template<typename CGlueInst, typename CGlueCtx>
struct (?P<group>{})Container) \{{
    (?P<fields>CGlueInst instance;
    CGlueCtx context;)
\}};",
        typenames,
    ))
    .map_err(Into::into)
}

fn maybe_uninit_regex() -> Result<Regex> {
    Regex::new(r"struct MaybeUninit;").map_err(Into::into)
}

fn trait_obj_regex() -> Result<Regex> {
    Regex::new(
        r"template<typename T, typename V, typename C, typename R>
struct CGlueTraitObj \{
    const V \*vtbl;
    CGlueObjContainer<T, C, R> container;
\};",
    )
    .map_err(Into::into)
}
