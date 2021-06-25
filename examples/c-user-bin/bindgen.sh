#!/bin/sh

OUT=${PWD}/bindings.h
CFG=${PWD}/cbindgen.toml

cd ../plugin-api

cglue-bindgen +nightly -- --config $CFG --crate plugin-api --output $OUT -l C++
