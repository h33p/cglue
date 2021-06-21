//! # cglue-bindgen
//!
//! Cleanup cbindgen output for CGlue.
//!
//! This crate essentially wraps cbindgen and performs additional header cleanup steps on top for
//! good out-of-the-box usage.
//!
//! ## Install
//!
//! ```sh
//! cargo install cglue-bindgen
//! ```
//!
//! Also make sure cbindgen is installed:
//!
//! ```sh
//! cargo install cbindgen
//! ```
//!
//! ## Running
//!
//! Run similarly to cbindgen:
//!
//! ```sh
//! cglue-bindgen +nightly -- --config cbindgen.toml --crate your_crate --output output_header.h
//! ```

use regex::*;
use std::env;
use std::fs::File;
use std::io::Write;
use std::process::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args = env::args()
        .into_iter()
        .skip_while(|v| v != "--")
        .collect::<Vec<_>>();

    // Hijack the output

    let mut output_file = None;
    let mut args_out = vec![];

    let mut windows = args.windows(2);

    while let Some(a) = windows.next() {
        // Skip the first arg, the "--", which also allows us to filter 2 args in one go when we
        // need that.
        match a[0].as_str() {
            "-o" | "--output" => {
                if output_file.is_none() {
                    output_file = Some(a[1].clone());
                }
            }
            _ => {
                if a[1] != "-o" && a[1] != "--output" {
                    args_out.push(a[1].clone());
                }
            }
        }
    }

    let use_nightly = env::args()
        .take_while(|v| v != "--")
        .any(|v| v == "+nightly");

    let mut cmd = if use_nightly {
        let mut cmd = Command::new("rustup");

        cmd.args(&["run", "nightly", "cbindgen"]);

        cmd
    } else {
        Command::new("cbindgen")
    };

    cmd.args(&args_out[..]);

    let output = cmd.output()?;

    if !output.status.success() {
        eprintln!("{}", std::str::from_utf8(&output.stderr)?);
        Err("cbindgen failed")?
    }

    let out = std::str::from_utf8(&output.stdout)?.to_string();

    let mut zero_sized_ret = vec![];
    // Matched in C mode for all zero-sized RetTmp structs
    let zsrc = Regex::new(r"^typedef struct (?P<trait>\w+)RetTmp (?P<trait2>\w+)RetTmp;")?;
    // Matched in C++ mode for all zero-sized RetTmp structs
    let zsr = Regex::new(r"^struct (?P<trait>\w+)RetTmp;")?;
    // Match for any RetTmp struct. Checked against the previous zsr matches.
    let field = Regex::new(r"\s+(?:struct | )(?P<trait>\w+)RetTmp ret_tmp(_\w+;|;)")?;
    // Matches any maybe uninit definitions in C++ mode.
    let maybe_uninit = Regex::new(r"^struct MaybeUninit;")?;
    // Matches any NoContext definitions.
    let no_context = Regex::new(r"^(struct NoContext|typedef struct NoContext NoContext);")?;
    // Matches any NoContext fields in C mode.
    let no_context_field = Regex::new(r"\s+struct NoContext ctx;")?;
    // Matches any NoContext pointer arguments C mode.
    let no_context_arg = Regex::new(r"struct NoContext \*")?;

    let mut output = vec![];

    for line in out.lines() {
        if zsrc.is_match(line) {
            // TODO: just remove it and remove the comment above.
            for m in zsrc.captures_iter(line) {
                zero_sized_ret.push(m[1].to_string());
            }
            let line = zsrc.replace_all(line, "typedef void ${trait}RetTmp;");
            output.push(line.to_string());
        } else if zsr.is_match(line) {
            for m in zsr.captures_iter(line) {
                zero_sized_ret.push(m[1].to_string());
            }
            let line = zsr.replace_all(line, "typedef void ${trait}RetTmp;");
            output.push(line.to_string());
        } else if let Some(cap) = field.captures(line) {
            if !zero_sized_ret.contains(&cap[1].to_string()) {
                output.push(line.to_string());
            }
        } else if maybe_uninit.is_match(line) {
            let line = maybe_uninit.replace_all(line, "using MaybeUninit = T;");
            output.push(line.to_string());
        } else if no_context.is_match(line) {
            let line = no_context.replace_all(line, "typedef void NoContext;");
            output.push(line.to_string());
        } else if no_context_arg.is_match(line) {
            let line = no_context_arg.replace_all(line, "void *");
            output.push(line.to_string());
        } else if !no_context_field.is_match(line) {
            output.push(line.to_string());
        }
    }

    let output = output.join("\n");

    // Finally, insert specialized implementations of the CtxBox and CGlueTraitObj structures.

    let output = output.replace(
        r"struct CGlueTraitObj {
    T instance;
    const V *vtbl;
    S ret_tmp;
};",
        r"struct CGlueTraitObj {
    T instance;
    const V *vtbl;
    S ret_tmp;
};

template<typename T, typename V>
struct CGlueTraitObj<T, V, void> {
    T instance;
    const V *vtbl;
};",
    );

    let output = output.replace(
        r"struct CtxBox {
    CBox<T> inner;
    C ctx;
};",
        r"struct CtxBox {
    CBox<T> inner;
    C ctx;
};

template<typename T>
struct CtxBox<T, void> {
    CBox<T> inner;
};",
    );

    if let Some(path) = output_file {
        let mut file = File::create(path)?;
        file.write_all(output.as_str().as_bytes())?;
    } else {
        print!("{}", output);
    }

    Ok(())
}
