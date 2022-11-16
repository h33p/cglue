//! # cglue-bindgen
//!
//! Cleanup cbindgen output for CGlue.
//!
//! This crate essentially wraps cbindgen and performs additional header cleanup steps on top for
//! good out-of-the-box usage. Note that the program expects standard naming convention, and will
//! likely break if there is any renaming happening in cbindgen config.
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
//!
//! ## Configuring
//!
//! Create a `cglue.toml`, and pass `-c cglue.toml` to `cglue-bindgen` before the `--`.
//!
//! Several values can be set:
//!
//! `default_container` - set the default container type. This will make C/C++ code less verbose
//! for objects that match the container and context types. Supports out-of-the-box:
//! `Box`, `Mut`, `Ref`.
//!
//! `default_context` - set the default context type. This will make C/C++ code less verbose for
//! objects that match the container and context types. Supports out-of-the-box: `Arc`,
//! `NoContext`.
//!
//! ## Using the bindings
//!
//! Check the documentation for the respective language:
//!
//! * [C](self::codegen::c)
//!
//! * [C++](self::codegen::cpp)
//!
//! You can also check the [code examples](https://github.com/h33p/cglue/tree/main/examples).
//!
//! ## In case of an issue
//!
//! Please check if any custom cbindgen options are influencing the way the code is generated in
//! any way. This crate is very finicky, and for instance, even changing the documentation style
//! is likely to break the code generation.
//!
//! If you still have issues without any custom parameters, please report an issue, because then it
//! is likely my fault or cbindgen update broke the binding generation.
//!
//! Verified to work cbindgen version: `v0.20.0`.
//!

use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::process::*;

pub mod types;
use types::Result;

pub mod codegen;
use codegen::{c, cpp};

pub mod config;
use config::Config;

fn main() -> Result<()> {
    let args_pre = env::args()
        .skip(1)
        .take_while(|v| v != "--")
        .collect::<Vec<_>>();
    let args = env::args().skip_while(|v| v != "--").collect::<Vec<_>>();

    // Hijack the output

    let mut output_file = None;
    let mut args_out = vec![];

    for a in args.windows(2) {
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

    let mut config = Config::default();

    for a in args_pre.windows(2) {
        match a[0].as_str() {
            "-c" | "--config" => {
                let mut f = File::open(&a[1])?;
                let mut val = vec![];
                f.read_to_end(&mut val)?;
                config = toml::from_str(std::str::from_utf8(&val)?)?;
            }
            _ => {}
        }
    }

    let use_nightly = args_pre.iter().any(|v| v == "+nightly");

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
        return Err("cbindgen failed".into());
    }

    let out = std::str::from_utf8(&output.stdout)?.to_string();

    let output = if cpp::is_cpp(&out)? {
        cpp::parse_header(&out, &config)?
    } else if c::is_c(&out)? {
        c::parse_header(&out, &config)?
    } else {
        return Err("Unsupported header format!".into());
    };

    if let Some(path) = output_file {
        let mut file = File::create(path)?;
        file.write_all(output.as_str().as_bytes())?;
    } else {
        print!("{}", output);
    }

    Ok(())
}
