//! # C code generator.
//!
//! This generator performs multiple things:
//!
//! ## Cleanup the types to compile properly.
//!
//! This cleanup is more substantial than in C++ side. Specifically, additional monomorphization
//! happens to replace all `Context` types with context structures that we find actually used. Zero
//! sized structures are being removed, and other missing types are defined.
//!
//! ## Generate trait function wrappers.
//!
//! Trait function wrappers are being created. If function's arguments do not matter from type to
//! type, a generic wrapper is emitted, that accepts a `void` pointer. Otherwise, a container and
//! context type prefix is being emitted, unless the type is specifically configured to be the
//! default one.
//!
//! ## Generate clone/drop functions.
//!
//! ## Add shortened typedefs for default types.
//!
//! Basically, `PluginInnerArcBox` drops the suffix and becomes just `PluginInner`.
//!
//! ## Add callback and iterator wrappers for simpler usage.
//!
//! Callback is able to collect data into a buffer that is either statically, or dynamically
//! allocated.
//!
//! Iterators can be built out of buffers easily.
//!
//! ## Define macros for simpler usage.
//!
//! `MUT_SLICE`, `REF_SLICE`, `STR` macros allow to build slices out of strings or buffers,
//! `COLLECT_CB` and its derivatives deal with buffer callbacks, `COUNT_CB` creates a callback that
//! simply counts elements, `BUF_ITER` and its derivatives deal with constructing iterators from
//! buffers.
//!

use crate::config::*;
use crate::types::*;
use itertools::Itertools;
use log::trace;
use regex::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub fn is_c(header: &str) -> Result<bool> {
    Ok(Regex::new(
        r"
typedef struct [^\s]+ \{|",
    )?
    .is_match(header))
}

pub fn parse_header(header: &str, config: &Config) -> Result<String> {
    // COLLECTION:

    // Collect all zsized ret tmps
    let zsr_regex = zero_sized_ret_regex()?;
    let zst_rets = zsr_regex
        .captures_iter(header)
        .map(|c| (c["trait"].to_string(), c["context"].to_string()))
        .collect::<Vec<_>>();

    // Collect all contexts
    let mut contexts = HashSet::new();

    for (cap, ctx) in &zst_rets {
        contexts.insert(ctx.clone());
        trace!("CAP: {} {}", cap, ctx);
    }

    // Check if we need to typedef `TypeLayout`
    let type_layout_re = Regex::new(r"((typedef [\s]+)|struct) TypeLayout")?;
    let needs_type_layout =
        header.contains("const TypeLayout *") && !type_layout_re.is_match(header);

    // PROCESSING:

    // Remove zsized ret tmps
    let header = zsr_regex.replace_all(header, "");

    let gr_regex = group_ret_tmp_regex(&zst_rets)?;
    let header = gr_regex.replace_all(&header, "");

    let cr_regex = ret_tmp_regex(&zst_rets)?;
    let header = cr_regex.replace_all(&header, "");

    // Monomorphize Contexts:
    contexts.remove("Context");
    let header = monomorphize_contexts(header, &contexts)?;

    // MORE COLLECTION:

    // Collect all vtables
    let vtbl_regex = vtbl_regex()?;

    let vtbls = vtbl_regex
        .captures_iter(&header)
        .map(|c| {
            (
                c["trait"].to_string(),
                c["container"].to_string(),
                c["context"].to_string(),
                c["functions"].to_string(),
            )
        })
        .collect::<Vec<_>>();

    let mut group_vtbls = vec![];
    let mut obj_vtbls = vec![];

    let obj_cont_regex =
        Regex::new("(?P<inner>[^\\s]+[^_])__(___)?(?P<ctx>[^_][^\\s]+[^_])__.+RetTmp_(?P<ctx2>)")?;
    let gobj_cont_regex = Regex::new("(?P<inner>[^\\s]+[^_])__(___)?(?P<ctx>[^_][^\\s]+[^_])")?;

    for (t, cont, ctx, funcs) in vtbls {
        if cont == "CGlueObjContainer" {
            let captures = obj_cont_regex
                .captures(&ctx)
                .ok_or("Failed to parse captures")?;

            obj_vtbls.push((
                t,
                ctx.to_string(),
                captures["inner"].to_string(),
                captures["ctx"].to_string(),
                funcs,
            ));
        } else {
            let captures = gobj_cont_regex
                .captures(&ctx)
                .ok_or("Failed to parse captures")?;
            group_vtbls.push((
                t,
                Regex::new("(?P<group>.*)Container")?
                    .replace(&cont, "$group")
                    .to_string(),
                ctx.to_string(),
                captures["inner"].to_string(),
                captures["ctx"].to_string(),
                funcs,
            ));
        }
    }

    let inner_map = ContainerType::get_map();

    let context_map = ContextType::get_map();

    // FINAL PROCESSING:

    // Remove any NoContext usage

    let header = Regex::new(r"\s+(struct )?NoContext context;\n")?.replace_all(&header, "");

    // Wrapper rules:
    //
    // wrapper is generated as {optional_prefix/group_name}_{name}_{cont_type}_{optional_ctx_type}
    //
    // cont_type is only included when the function consumes self
    //
    // ctx_type is ommitted when it is void, or the return type does not contain context (and the
    // function does not consume self).

    let mut all_wrappers = String::new();
    let mut fwd_declarations = String::new();
    let mut shortened_typedefs = vec![];

    // Create context clone/drop wrappers
    for (ty, prefix, impl_clone, impl_drop) in context_map.iter().filter_map(
        |(
            ty,
            ContextType {
                ty_prefix,
                clone_impl,
                drop_impl,
                ..
            },
        )| {
            match (clone_impl, drop_impl) {
                (Some(impl_clone), Some(impl_drop)) => {
                    Some((ty, ty_prefix.to_lowercase(), impl_clone, impl_drop))
                }
                _ => None,
            }
        },
    ) {
        all_wrappers += &format!(
            r"static inline {ty} ctx_{prefix}_clone({ty} *self) {{
    {ty} ret = *self;
    {impl_clone}
    return ret;
}}

static inline void ctx_{prefix}_drop({ty} *self) {{
    {impl_drop}
}}
",
            ty = ty,
            prefix = prefix,
            impl_clone = impl_clone,
            impl_drop = impl_drop
        );
    }

    for (ty, prefix, impl_drop) in inner_map.iter().filter_map(
        |(
            ty,
            ContainerType {
                ty_prefix,
                drop_impl,
                ..
            },
        )| drop_impl.map(|impl_drop| (ty, ty_prefix.to_lowercase(), impl_drop)),
    ) {
        all_wrappers += &format!(
            r"static inline void cont_{prefix}_drop({ty} *self) {{
    {impl_drop}
}}
",
            ty = ty,
            prefix = prefix,
            impl_drop = impl_drop
        );
    }

    // Create wrappers to single trait objects

    let mut generated_funcs = Default::default();

    let mut vtbl_types: HashMap<String, HashSet<String>> = HashMap::new();

    for (t, second_half, _, _, funcs) in &obj_vtbls {
        let container_ty = format!("struct CGlueObjContainer_{}", second_half);

        let vtbl = Vtable::new(t.to_string(), funcs, &container_ty)?;

        for f in vtbl.functions {
            vtbl_types.entry(f.name).or_default().insert(t.clone());
        }
    }

    for (t, second_half, inner, context, funcs) in obj_vtbls {
        trace!("{}Vtbl_CGlueObjContainer_{}", t, second_half);

        let obj_regex = obj_regex(&format!("{}Vtbl_CGlueObjContainer_{}", t, second_half))?;

        let this_ty = &obj_regex
            .captures(&header)
            .ok_or("Unable to find trait obj")?["objtype"];

        let this_ty = format!("struct {}", this_ty);
        let container_ty = format!("struct CGlueObjContainer_{}", second_half);

        fwd_declarations += &format!("{};\n", this_ty);
        fwd_declarations += &format!("{};\n", container_ty);

        let vtbl = Vtable::new(t.clone(), &funcs, &container_ty)?;

        let ContainerType {
            ty_prefix: cont,
            drop_impl: container_wrappers,
            ..
        } = inner_map
            .get(inner.as_str())
            .copied()
            .unwrap_or_else(|| ContainerType::from_name(inner.as_str()));
        let ContextType {
            ty_prefix: ctx,
            drop_impl: context_wrappers,
            ..
        } = context_map
            .get(context.as_str())
            .copied()
            .unwrap_or_else(|| ContextType::from_name(context.as_str()));

        let wrappers = vtbl.create_wrappers_c(
            ("container", "vtbl"),
            ("", &|f| {
                if f.name == "drop" || vtbl_types.get(&f.name).unwrap().len() > 1 {
                    Some(&t)
                } else {
                    None
                }
            }),
            (&container_ty, cont, container_wrappers.is_some()),
            (&context, ctx, context_wrappers.is_some()),
            (&this_ty, &[]),
            &mut generated_funcs,
            config,
        );

        if config.default_context.as_deref() == Some(ctx)
            && config.default_container.as_deref() == Some(cont)
        {
            shortened_typedefs.push((this_ty, t));
        }

        all_wrappers += &wrappers;
    }

    // Create wrappers to group objects

    for (t, cont, second_half, inner, context, funcs) in group_vtbls {
        let this_ty = format!("struct {}_{}", cont, second_half);
        let container_ty = format!("struct {}Container_{}", cont, second_half);

        fwd_declarations += &format!("{};\n", this_ty);
        fwd_declarations += &format!("{};\n", container_ty);

        let vtbl = Vtable::new(t, &funcs, &container_ty)?;

        let ContainerType {
            ty_prefix: inner,
            drop_impl: container_wrappers,
            ..
        } = inner_map
            .get(inner.as_str())
            .copied()
            .unwrap_or_else(|| ContainerType::from_name(inner.as_str()));
        let ContextType {
            ty_prefix: ctx,
            drop_impl: context_wrappers,
            ..
        } = context_map
            .get(context.as_str())
            .copied()
            .unwrap_or_else(|| ContextType::from_name(context.as_str()));

        let wrappers = vtbl.create_wrappers_c(
            ("container", &format!("vtbl_{}", vtbl.name.to_lowercase())),
            ("", &|_| Some(&cont)),
            (&container_ty, inner, container_wrappers.is_some()),
            (&context, ctx, context_wrappers.is_some()),
            (&this_ty, &[]),
            &mut generated_funcs,
            config,
        );

        if config.default_context.as_deref() == Some(ctx)
            && config.default_container.as_deref() == Some(inner)
        {
            shortened_typedefs.push((this_ty, cont.to_string()));
        }

        all_wrappers += &wrappers;
    }

    let mut header = header.to_string();

    let strip_root_regex = Regex::new("(struct )?(?P<root>.*)")?;

    // Add shortened typedefs for config
    for (root, ty) in shortened_typedefs {
        if Regex::new(&format!(r"typedef (struct )?[^\s]+ {};", ty))?.is_match(&header) {
            continue;
        }

        let root = strip_root_regex.replace(&root, "$root");

        let regex = Regex::new(&format!(
            r"(/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
)?typedef (?P<old_ty_def>struct {root})( \{{[^\}}]*\}})? {root};",
            root = root
        ))?;

        header = regex
            .replace(
                &header,
                format!(
                    r"$0

// Typedef for default container and context type
typedef struct {} {};",
                    root, ty
                ),
            )
            .into();
    }

    // Create callback and iterator wrappers

    all_wrappers += r"
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
";

    for caps in callback_regex()?.captures_iter(&header) {
        all_wrappers += &format!(
            r"
static inline bool cb_collect_static_{typename}(struct CollectBase *ctx, {typename} info) {{
    return cb_collect_static_base(ctx, sizeof({typename}), &info);
}}

static inline bool cb_collect_dynamic_{typename}(struct CollectBase *ctx, {typename} info) {{
    return cb_collect_dynamic_base(ctx, sizeof({typename}), &info);
}}

static inline bool cb_count_{typename}(size_t *cnt, {typename} info) {{
    return ++(*cnt);
}}
",
            typename = &caps["typename"]
        );
    }

    // Also define helper macros

    let helper_macros = r"// Construct a typed slice for rust functions
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
";

    // Insert forward decls at the start

    let fwd_declarations = if fwd_declarations.is_empty() {
        format!("$0\n{}", helper_macros)
    } else {
        format!(
            "$0\n{}\n// Forward declarations for vtables and their wrappers\n{}",
            helper_macros, fwd_declarations
        )
    };

    let fwd_declarations = if needs_type_layout {
        format!(
            "{}\ntypedef struct TypeLayout TypeLayout;\n",
            fwd_declarations
        )
    } else {
        fwd_declarations
    };

    // TODO: improve start detection
    let header = Regex::new("(#ifndef [^\n]+\n#define [^\n]+\n|.*(#include [^\n]+\n)+)|^")?
        .replace(&header, fwd_declarations);

    // Insert the wrappers at the end

    all_wrappers += "\n$0";

    let header = Regex::new(
        "(
#ifdef __cplusplus
} // extern \"C\"
#endif // __cplusplus|$)",
    )?
    .replace(&header, all_wrappers);

    Ok(header.to_string())
}

fn monomorphize_contexts(
    header: std::borrow::Cow<str>,
    contexts: &HashSet<String>,
) -> Result<std::borrow::Cow<'static, str>> {
    // Replace all structs that end with _Context with N specifications that use individual context
    // Within them, replace all types that have _Context with the said context.
    // Replace `Context context;` with specific context.

    let ctx_matches =
        Itertools::intersperse(contexts.iter().map(String::as_str), "|").collect::<String>();

    let context_match = "Context context;";

    let cfield_regex = Regex::new("(?P<typename>.+)_Context")?;

    let cstruct_regex = Regex::new(
        r"(?P<comment>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
)?typedef struct (?P<name>.+)_Context(?P<inside>( \{[^\}]*\} | ))(?P<name2>[^;]+)_Context;

",
    )?;

    let header = cstruct_regex.replace_all(&header, |caps: &Captures| {
        let mut ret = String::new();

        let name = caps["name"].to_string();
        let name2 = caps["name2"].to_string();

        for context in contexts {
            // TODO: is this always correct?? Seems to be for CArc_c_void
            let context_ty = format!("_{}____", context);

            let name = name.replace("_Context_", &context_ty);
            let name2 = name2.replace("_Context_", &context_ty);

            if header.contains(&format!("typedef struct {}_{}", name, context)) {
                continue;
            }

            ret.push_str(&format!(
                r"{}typedef struct {}_{}{}{}_{};
",
                caps.name("comment").map(|m| m.as_str()).unwrap_or(""),
                name,
                context,
                cfield_regex
                    .replace_all(&caps["inside"], |caps: &Captures| {
                        let ty = caps["typename"]
                            .to_string()
                            .replace("_Context_", &context_ty);
                        format!("{}_{}", ty, context)
                    })
                    .replace(context_match, &format!("{} context;", context)),
                name2,
                context
            ));
        }

        ret
    });

    // Go through each struct that matches a context, and replace all types within that have _Context with parent type

    let cstruct_regex = Regex::new(&format!(
        r"(?P<comment>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
)?typedef struct (?P<name>.+)_(?P<context>{}) \{{
(?P<inside>[^\}}]*)
\}} (?P<fulltype>[^;]+);
",
        ctx_matches
    ))?;

    let mut header = cstruct_regex
        .replace_all(&header, |caps: &Captures| {
            let context = &caps["context"];

            format!(
                r"{}typedef struct {}_{} {{
{}
}} {}_{};
",
                caps.name("comment").map(|m| m.as_str()).unwrap_or(""),
                &caps["name"],
                context,
                cfield_regex.replace_all(&caps["inside"], &format!("${{typename}}_{}", context)),
                &caps["name"],
                context
            )
        })
        .to_string();

    let mut types_to_explore = VecDeque::new();

    // Go through each struct that matches a context, find any of its typedefs, and move them after
    // the said struct to avoid incomplete types.

    for cap in cstruct_regex.captures_iter(&header) {
        types_to_explore.push_back((
            cap["fulltype"].to_string(),
            format!("struct {}", &cap["fulltype"]),
            cap["fulltype"].to_string(),
        ));
    }

    while let Some((root, old_ty, ty)) = types_to_explore.pop_front() {
        // Check if the type has users beforehand. If so, move the root above there and restart the
        // flow...
        let ty_regex = Regex::new(&format!(
            r"
(?P<user_typedef>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
typedef struct .+ \{{[^\}}]*{ty}[^\}}]*\}} .+;
)(?P<inbetween>[\s\S]*)
(?P<ty_typedef>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/
typedef (?P<old_ty_def>struct {old_ty})( \{{[^\}}]*\}})? {ty};
)",
            ty = ty,
            old_ty = old_ty
        ))?;

        let mut matched = false;

        // TODO: move non-ptr objects referenced by struct above it

        header = ty_regex
            .replace(&header, |caps: &Captures| {
                matched = true;

                format!(
                    "{}{}{}",
                    &caps["ty_typedef"], &caps["inbetween"], &caps["user_typedef"]
                )
            })
            .into();

        if matched {
            types_to_explore.push_back((root, old_ty, ty));
            continue;
        }

        let typedef_regex = Regex::new(&format!(
            r"(?P<old_typedef>(/\*[^*]*\*+(?:[^/*][^*]*\*+)*/)?
typedef (?P<old_ty>(struct )?{old_ty})( \{{[^\}}]*\}})? {ty};
)(?P<inbetween>[\s\S]*)
(?P<new_typedef>(/\*[^*]*\*+(?:[^/*][^*]*\*+)*/)
typedef (struct )?{ty} (?P<new_ty>.+);
)",
            old_ty = old_ty,
            ty = ty
        ))?;

        header = typedef_regex
            .replace(&header, |caps: &Captures| {
                let new_ty = caps["new_ty"].to_string();
                types_to_explore.push_front((root.clone(), ty.clone(), new_ty));
                format!(
                    "{}{}{}",
                    &caps["old_typedef"], &caps["new_typedef"], &caps["inbetween"]
                )
            })
            .into();
    }

    // Move all mentioned context types above their first use to avoid incomplete types.

    for context in contexts {
        let ctx_def_regex = Regex::new(&format!(
            r"(/\\*[^*]*\\*+(?:[^/*][^*]*\\*+)*/
)?typedef struct {context} \{{
[^\}}]*
\}} {context};
",
            context = context
        ))?;

        let ctx_user_regex = Regex::new(&format!(
            r"(?P<comment>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/)?
typedef struct .+_{} \{{
(?P<inside>[^\}}]*)
\}} .+_{};
",
            context, context
        ))?;

        if let Some(mut m) = ctx_def_regex.find(&header).map(|m| m.as_str().to_string()) {
            m.push_str("$0");

            header = ctx_def_regex.splitn(&header, 2).collect::<String>();
            header = ctx_user_regex.replace(&header, &m).to_string();
        }
    }

    Ok(header.into())
}

fn vtbl_regex() -> Result<Regex> {
    Regex::new(
        r"(?P<declaration>/\*\*
 \* CGlue vtable for trait (?P<trait2>\w+).
 \*
 \* This virtual function table contains ABI-safe interface for the given trait.
 \*/
typedef struct (?P<trait>\w+)Vtbl)_(?P<container>.*Container)_(?P<context>[^\s]*) \{
    (?P<functions>[^\}]+)
\} (.*)Vtbl_[^;]+;",
    )
    .map_err(Into::into)
}

fn obj_regex(vtbl: &str) -> Result<Regex> {
    Regex::new(&format!(
        r"/\*\*
 \* Simple CGlue trait object\.
 \*
 \* This is the simplest form of CGlue object, represented by a container and vtable for a single
 \* trait\.
 \*
 \* Container merely is a this pointer with some optional temporary return reference context\.
 \*/
typedef struct (?P<objtype>CGlueTraitObj_[^\s]+_{}_[^\s]+) \{{",
        vtbl
    ))
    .map_err(Into::into)
}

fn callback_regex() -> Result<Regex> {
    Regex::new(
        r"typedef struct Callback_c_void__(?P<typename>[^\s]+) \{[^}]*\} Callback_c_void__[^\s]+;",
    )
    .map_err(Into::into)
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
typedef struct (?P<trait>\w+)RetTmp_(?P<context>.+) \w+RetTmp_.+;
",
    )
    .map_err(Into::into)
}

fn group_ret_tmp_regex(zero_sized: &[(String, String)]) -> Result<Regex> {
    let typenames = Itertools::intersperse(
        zero_sized.iter().map(|(a, b)| format!("{}RetTmp_{}", a, b)),
        "|".to_string(),
    )
    .collect::<String>();
    let typenames_lc = Itertools::intersperse(
        zero_sized.iter().map(|(a, _)| a.to_lowercase()),
        "|".to_string(),
    )
    .collect::<String>();
    Regex::new(&format!(
        "\\s*struct ({}) ret_tmp_({});",
        typenames, typenames_lc
    ))
    .map_err(Into::into)
}

fn ret_tmp_regex(zero_sized: &[(String, String)]) -> Result<Regex> {
    let typenames = Itertools::intersperse(
        zero_sized.iter().map(|(a, b)| format!("{}RetTmp_{}", a, b)),
        "|".to_string(),
    )
    .collect::<String>();
    Regex::new(&format!("\\s*struct ({}) ret_tmp;", typenames)).map_err(Into::into)
}
