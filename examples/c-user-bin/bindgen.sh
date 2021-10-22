#!/bin/sh

OUT=${PWD}/bindings.h
CFG=${PWD}/cbindgen.toml

cd ../plugin-api

../../target/release/cglue-bindgen +nightly -- --config $CFG --crate plugin-api --output $OUT -l C
