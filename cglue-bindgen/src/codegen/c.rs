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

    // Monomorphize Contexts:
    contexts.remove("Context");
    let header = monomorphize_contexts(header, &contexts)?;

    // MORE COLLECTION:

    // Collect all vtables

    // Collect groups

    // FINAL PROCESSING:

    // Wrapper rules:
    //
    // wrapper is generated as {optional_prefix/group_name}_{cont_type}_{ctx_type}_{name}
    //
    // cont_type is only included when the function consumes self
    //
    // ctx_type is ommitted when it is void, or the return type does not contain context (and the
    // function does not consume self).

    // Create wrappers to group objects

    // Create wrappers to single trait objects

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
typedef struct (?P<name>.+)_Context \{
(?P<inside>[^\}]*)
\} (?P<fulltype>[^;]+);",
    )?;

    let header = cstruct_regex.replace_all(&header, |caps: &Captures| {
        let mut ret = String::new();

        for context in contexts {
            if header.contains(&format!("typedef struct {}_{}", &caps["name"], context)) {
                continue;
            }

            ret.push_str(&format!(
                r"{}
typedef struct {}_{} {{
{}
}} {}_{};",
                caps.name("comment").map(|m| m.as_str()).unwrap_or(""),
                &caps["name"],
                context,
                context_regex.replace_all(
                    &cfield_regex.replace_all(
                        &caps["inside"],
                        &format!("struct ${{typename}}_{}", context)
                    ),
                    format!("{} context;", context)
                ),
                &caps["name"],
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
