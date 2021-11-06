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

use std::env;
use std::fs::File;
use std::io::Write;
use std::process::*;

pub mod types;
use types::Result;

pub mod codegen;
use codegen::{c, cpp};

fn main() -> Result<()> {
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
        return Err("cbindgen failed".into());
    }

    let out = std::str::from_utf8(&output.stdout)?.to_string();

    let output = if cpp::is_cpp(&out)? {
        cpp::parse_header(&out)?
    } else if c::is_c(&out)? {
        c::parse_header(&out)?
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
