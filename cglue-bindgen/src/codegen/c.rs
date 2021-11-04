use crate::types::*;
use itertools::Itertools;
use regex::*;
use std::collections::{HashMap, HashSet};

pub fn parse_header(header: &str) -> Result<String> {
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
        println!("CAP: {} {}", cap, ctx);
    }

    // PROCESSING:

    // Remove zsized ret tmps
    let header = zsr_regex.replace_all(&header, "");

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

    let inner_map = [
        (
            "CBox_c_void",
            ("box", Some("self->drop_fn(self->instance);")),
        ),
        ("____c_void", ("mut", None)),
        ("_____c_void", ("ref", None)),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<_, _>>();

    let context_map = [
        ("NoContext", ("", None)),
        (
            "COptArc_c_void",
            (
                "arc",
                Some((
                    "ret.instance = self->clone_fn(self->instance);",
                    "self->drop_fn(self->instance);",
                )),
            ),
        ),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<_, _>>();

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

    // Create context clone/drop wrappers
    for (ty, prefix, impl_clone, impl_drop) in
        context_map.iter().filter_map(|(ty, (prefix, imp))| {
            imp.map(|(impl_clone, impl_drop)| (ty, prefix, impl_clone, impl_drop))
        })
    {
        all_wrappers += &format!(
            r"{ty} ctx_{prefix}_clone({ty} *self) {{
    {ty} ret = *self;
    {impl_clone}
    return ret;
}}

void ctx_{prefix}_drop({ty} *self) {{
    {impl_drop}
}}
",
            ty = ty,
            prefix = prefix,
            impl_clone = impl_clone,
            impl_drop = impl_drop
        );
    }

    for (ty, prefix, impl_drop) in inner_map
        .iter()
        .filter_map(|(ty, (prefix, imp))| imp.map(|impl_drop| (ty, prefix, impl_drop)))
    {
        all_wrappers += &format!(
            r"void cont_{prefix}_drop({ty} *self) {{
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

        let vtbl = Vtable::new(t.to_string(), &funcs, &container_ty)?;

        for f in vtbl.functions {
            vtbl_types.entry(f.name).or_default().insert(t.clone());
        }
    }

    for (t, second_half, inner, context, funcs) in obj_vtbls {
        println!("{}Vtbl_CGlueObjContainer_{}", t, second_half);

        let obj_regex = obj_regex(&format!("{}Vtbl_CGlueObjContainer_{}", t, second_half))?;

        let this_ty = &obj_regex
            .captures(&header)
            .ok_or("Unable to find trait obj")?["objtype"];

        //let this_ty = format!("struct CGlueTraitObj_{}", second_half);
        let container_ty = format!("struct CGlueObjContainer_{}", second_half);

        let vtbl = Vtable::new(t.clone(), &funcs, &container_ty)?;

        let (cont, container_wrappers) = inner_map
            .get(inner.as_str())
            .copied()
            .unwrap_or((inner.as_str(), None));
        let (ctx, context_wrappers) = context_map
            .get(context.as_str())
            .copied()
            .unwrap_or((context.as_str(), None));

        let wrappers = vtbl.create_wrappers_c(
            "container",
            "vtbl",
            &"",
            &|f| {
                if f.name == "drop" || vtbl_types.get(&f.name).unwrap().len() > 1 {
                    Some(&t)
                } else {
                    None
                }
            },
            &this_ty,
            (&container_ty, cont, container_wrappers.is_some()),
            (&context, ctx, context_wrappers.is_some()),
            &[],
            &mut generated_funcs,
        );

        all_wrappers += &wrappers;
    }

    // Create wrappers to group objects

    for (t, cont, second_half, inner, context, funcs) in group_vtbls {
        let this_ty = format!("struct {}_{}", cont, second_half);
        let container_ty = format!("struct {}Container_{}", cont, second_half);

        let vtbl = Vtable::new(t, &funcs, &container_ty)?;

        let (inner, container_wrappers) = inner_map
            .get(inner.as_str())
            .copied()
            .unwrap_or((&inner.as_str(), None));
        let (ctx, context_wrappers) = context_map
            .get(context.as_str())
            .copied()
            .unwrap_or((context.as_str(), None));

        let wrappers = vtbl.create_wrappers_c(
            "container",
            &format!("vtbl_{}", vtbl.name.to_lowercase()),
            "",
            &|_| Some(&cont),
            &this_ty,
            (&container_ty, &inner, container_wrappers.is_some()),
            (&context, ctx, context_wrappers.is_some()),
            &[],
            &mut generated_funcs,
        );

        all_wrappers += &wrappers;
    }

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

    let context_regex = Regex::new("Context context;")?;

    let cfield_regex = Regex::new("struct (?P<typename>.+)_Context")?;

    let cstruct_regex = Regex::new(
        r"(?P<comment>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/)?
typedef struct (?P<name>.+)_Context(?P<inside>( \{[^\}]*\} | ))(?P<name2>[^;]+)_Context;",
    )?;

    let header = cstruct_regex.replace_all(&header, |caps: &Captures| {
        let mut ret = String::new();

        let name = caps["name"].to_string();
        let name2 = caps["name2"].to_string();

        for context in contexts {
            let context_ty = format!("_{}_", context);

            let name = name.replace("_Context_", &context_ty);
            let name2 = name2.replace("_Context_", &context_ty);

            if header.contains(&format!("typedef struct {}_{}", name, context)) {
                continue;
            }

            ret.push_str(&format!(
                r"{}
typedef struct {}_{}{}{}_{};",
                caps.name("comment").map(|m| m.as_str()).unwrap_or(""),
                name,
                context,
                context_regex.replace_all(
                    &cfield_regex.replace_all(&caps["inside"], |caps: &Captures| {
                        let ty = caps["typename"]
                            .to_string()
                            .replace("_Context_", &context_ty);
                        format!("struct {}_{}", ty, context)
                    }),
                    format!("{} context;", context)
                ),
                name2,
                context
            ));
        }

        ret
    });

    // Go through each struct that matches a context, and replace all types within that have _Context with parent type

    let cstruct_regex = Regex::new(&format!(
        r"(?P<comment>/\*[^*]*\*+(?:[^/*][^*]*\*+)*/)?
typedef struct (?P<name>.+)_(?P<context>{}) \{{
(?P<inside>[^\}}]*)
\}} (?P<fulltype>[^;]+);",
        ctx_matches
    ))?;

    let mut header = cstruct_regex
        .replace_all(&header, |caps: &Captures| {
            let context = &caps["context"];

            format!(
                r"{}
typedef struct {}_{} {{
{}
}} {}_{};",
                caps.name("comment").map(|m| m.as_str()).unwrap_or(""),
                &caps["name"],
                context,
                cfield_regex.replace_all(
                    &caps["inside"],
                    &format!("struct ${{typename}}_{}", context)
                ),
                &caps["name"],
                context
            )
        })
        .to_string();

    // Move all mentioned context types above their first use to avoid incomplete types.

    for context in contexts {
        let ctx_def_regex = Regex::new(&format!(
            r"(/\\*[^*]*\\*+(?:[^/*][^*]*\\*+)*/)?
typedef struct {} \{{
[^\}}]*
\}} {};
",
            context, context
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

    Ok(header.to_string().into())
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

fn groups_regex(vtbls: &[Vtable], explicit_group: Option<&str>) -> Result<Regex> {
    let group_fmt = explicit_group.unwrap_or("\\w+");

    let vtbl_names =
        Itertools::intersperse(vtbls.iter().map(|v| v.name.as_str()), "|").collect::<String>();

    Regex::new(
        &format!(r"(?P<definition_start> \* `as_ref_`, and `as_mut_` functions obtain references to safe objects, but do not
 \* perform any memory transformations either. They are the safest to use, because
 \* there is no risk of accidentally consuming the whole object.
 \*/
struct (?P<group>{group_fmt})_(?P<container>[\w_]+)_(?P<context>[\w_]+) \{{
    (?P<vtbls>(\s*const struct ({vtbl_names})Vtbl<.*> \*vtbl_\w+;)*)
    (?P<group2>\w+)Container_(?P<container2>[\w_]+)_(?P<context2>[\w_]+) container;)
\}} (?P<group3>{group_fmt})_(?P<container3>[\w_]+)_(?P<context3>[\w_]+);", group_fmt = group_fmt, vtbl_names = vtbl_names),
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
