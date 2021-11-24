#!/bin/sh

OUT=${PWD}/bindings.h
CFG=${PWD}/cbindgen.toml
CGLUE_CFG=${PWD}/cglue.toml

cd ../plugin-api

../../target/release/cglue-bindgen +nightly -c $CGLUE_CFG -- --config $CFG --crate plugin-api --output $OUT -l C
