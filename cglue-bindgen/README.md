# cglue-bindgen

Cleanup cbindgen output for CGlue.

This crate essentially wraps cbindgen and performs additional header cleanup steps on top for
good out-of-the-box usage.

## Install

```sh
cargo install cglue-bindgen
```

Also make sure cbindgen is installed:

```sh
cargo install cbindgen
```

## Running

Run similarly to cbindgen:

```sh
cglue-bindgen +nightly -- --config cbindgen.toml --crate your_crate --output output_header.h
```

License: MIT
