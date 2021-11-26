# cglue-bindgen

Cleanup cbindgen output for CGlue.

This crate essentially wraps cbindgen and performs additional header cleanup steps on top for
good out-of-the-box usage. Note that the program expects standard naming convention, and will
likely break if there is any renaming happening in cbindgen config.

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

## Configuring

Create a `cglue.toml`, and pass `-c cglue.toml` to `cglue-bindgen` before the `--`.

Several values can be set:

`default_container` - set the default container type. This will make C/C++ code less verbose
for objects that match the container and context types. Supports out-of-the-box:
`Box`, `Mut`, `Ref`.

`default_context` - set the default context type. This will make C/C++ code less verbose for
objects that match the container and context types. Supports out-of-the-box: `Arc`,
`NoContext`.

## Using the bindings

Check the documentation for the respective language:

* [C](self::codegen::c)

* [C++](self::codegen::cpp)

You can also check the [code examples](https://github.com/h33p/cglue/tree/main/examples).

## In case of an issue

Please check if any custom cbindgen options are influencing the way the code is generated in
any way. This crate is very finicky, and for instance, even changing the documentation style
is likely to break the code generation.

If you still have issues without any custom parameters, please report an issue, because then it
is likely my fault or cbindgen update broke the binding generation.

Verified to work cbindgen version: `v0.20.0`.

