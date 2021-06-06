
# CGlue

[![Crates.io](https://img.shields.io/crates/v/cglue.svg)](https://crates.io/crates/cglue)
[![API Docs](https://docs.rs/cglue/badge.svg)](https://docs.rs/cglue)
[![Build and test](https://github.com/h33p/cglue/actions/workflows/build.yml/badge.svg)](https://github.com/h33p/cglue/actions/workflows/build.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/h33p/cglue/blob/main/LICENSE)
![Rustc 1.45](https://img.shields.io/badge/rustc-1.45+-lightgray.svg)

If all code is glued together, our glue is the safest on the market.

## FFI-safe trait generation, helper structures, and more!

<!-- toc -->
- [Overview](#overview)
- [In-depth look](#in-depth-look)
  - [Safety assumptions](#safety-assumptions)
  - [Name generation](#name-generation)
  - [Type wrapping](#type-wrapping)
  - [Associated type wrapping](#associated-type-wrapping)
  - [Working with cbindgen](#working-with-cbindgen)
    - [Setup](#setup)
    - [Cleanup C](#cleanup-c)
    - [Cleanup C++](#cleanup-c-1)
- [Limitations](#limitations)
<!-- /toc -->

## Overview

CGlue offers an easy way to ABI (application binary interface) safety. Just a few annotations and your trait is ready to go!

```rust
use cglue::*;

#[cglue_trait]
pub trait InfoPrinter {
    fn print_info(&self);
}

struct Info {
    value: usize
}

impl InfoPrinter for Info {
    fn print_info(&self) {
        println!("Info struct: {}", self.value);
    }
}

fn use_info_printer(printer: &impl InfoPrinter) {
    println!("Printing info:");
    printer.print_info();
}

fn main() {
    let mut info = Info {
        value: 5
    };

    let obj = trait_obj!(&mut info as InfoPrinter);

    use_info_printer(&obj);
}
```

A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.

This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `trait_obj` macro:

```rust
let obj = CGlueTraitObjInfoPrinter::from(&mut info).into_opaque();
```

`cglue_trait` annotation generates a `CGlueVtblInfoPrinter` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.

But that's not all, you can also group traits together!

```rust
use cglue::*;

#[cglue_trait]
pub trait InfoChanger {
    fn change_info(&mut self, new_val: usize);
}

impl InfoChanger for Info {
    fn change_info(&mut self, new_val: usize) {
        self.value = new_val;
    }
}

#[cglue_trait]
pub trait InfoDeleter {
    fn delete_info(&mut self);
}

// Define a trait group with `InfoPrinter` as mandatory trait, and
// `InfoChanger` with `InfoDeleter` as optional traits.
cglue_trait_group!(InfoGroup, InfoPrinter, { InfoChanger, InfoDeleter });

// Implement the group for `Info` structure, defining
// only that `InfoChanger` is optionally implemented.
cglue_impl_group!(Info, InfoGroup, InfoChanger);

let mut info = Info { value: 5 };

let mut obj = group_obj!(info as InfoGroup);

// Object does not implement `InfoDeleter`
assert!(as_ref!(&obj impl InfoDeleter).is_none());

change_info(&mut cast!(obj impl InfoChanger).unwrap(), 20);

fn change_info(change: &mut (impl InfoPrinter + InfoChanger), new_val: usize) {
    println!("Old info:");
    change.print_info();
    change.change_info(new_val);
    println!("New info:");
    change.print_info();
}
```

And there is much more! Here are some highlights:

1. Ability to use self-consuming trait functions.

2. Some standard library traits are exposed (`Clone`).

3. Ability to wrap associated trait types into new CGlue trait objects and groups.

4. The above ability also works with mutable, and const reference associated type returns*.

5. Generic traits and their groups.

## In-depth look

### Safety assumptions

This crate relies on encapsulation and the assumption that opaque objects will not be
tampered with, that is vtable functions will not be modified. For this reason, vtable
fields are not public, and neither are references on generated group objects. However,
it is still possible to access vtable references of generated groups from the same module.

`TODO: generate everything in a submodule?`

Essentially, this is the safety situation:

1. Destroying type information of trait objects and groups is safe, so long as objects and
   their vtables do not get swapped.

2. It is still possible to do that from the module that generates the group.

### Name generation

`#[cglue_trait]` macro for `MyTrait` will generate the following important types:

| Name | Purpose |
--- | ---
| CGlueBoxMyTrait | Typedef for opaque owned CGlue object. Its container is a [`CBox`](crate::boxed::CBox) |
| CGlueMutMyTrait | Typedef for opaque by-mut-ref CGlue object. Its container is a `&mut c_void`. |
| CGlueRefMyTrait | Typedef for opaque by-ref (const) CGlue object. Its container is a `&c_void`. |
| CGlueBaseMyTrait | Base typedef for a CGlue object. It allows for any container type, and is not opaque. |
| CGlueVtblMyTrait | Table of all functions of the trait. Should be opaque to the user. |
| OpaqueCGlueVtblMyTrait | Opaque version of the table. This is the type every object's table will have. |
| CGlueRetTmpMyTrait | Structure for temporary return values. It should be opaque to the user. |

`cglue_trait_group!` macro for `MyGroup` will generate the following main types:

| Name | Purpose |
--- | ---
| MyGroup | Base definiton of the group. It is not opaque and not usable yet. |
| MyGroupBox | Typedef for opaque owned CGlue trait group. Its container is a [`CBox`](crate::boxed::CBox) |
| MyGroupMut | Typedef for opaque by-mut-ref CGlue trait group. Its container is a `&mut c_void`. |
| MyGroupRef | Typedef for opaque by-ref (const) CGlue trait group. Its container is a `&c_void`. |
| MyGroupOpaque | Typedef for opaque CGlue trait group. It can have any container. |
| MyGroupVtableFiller | Trait that allows an object to specify which optional traits are available, through the use of `enable_trait` functions. |

The macro generation will also generate structures for all combinations of optional traits
being used. For more convenient by-macro usage, the names of optional traits inside are sorted
in alphabetical order. If not using macros, check `MyGroup` documentation for underlying
conversion function definitions.

### Type wrapping

As for details, commonly used Rust structures are automatically wrapped in a way that works
effectively.

For instance, slices get split up into pointer and size pairs:

```rust
fn with_slice(&self, slice: &[usize]) {}

// Generated vtable entry:

with_slice: extern "C" fn(&CGlueF, slice: *const usize, slice_size: usize),
```

`Option` types that can not have [nullable pointer optimization](https://doc.rust-lang.org/nomicon/ffi.html#the-nullable-pointer-optimization) are wrapped into [COption](crate::option::COption):

```rust
fn non_npo_option(&self, opt: Option<usize>) {}

// Generated vtable entry:

non_npo_option: extern "C" fn(&CGlueF, opt: Option<usize>),
```

`Result` is automatically wrapped into [CResult](crate::result::CResult):

```rust
fn with_cresult(&self) -> Result<usize, ()> {}

// Generated vtable entry:

with_cresult: extern "C" fn(&CGlueF) -> CResult<usize, ()>,
```

`Result` with [IntError](crate::result::IntError) type can return an integer code with `Ok` value written to a variable:

```rust
#[int_result]
fn with_int_result(&self) -> Result<usize> {}

// Generated vtable entry:

with_int_result: extern "C" fn(&CGlueF, ok_out: &mut MaybeUninit<usize>) -> i32,
```

All wrapping and conversion is handled transparently behind the scenes, with user's control.

### Associated type wrapping

Associated types can be wrapped into custom CGlue objects. Below is a minimal example of
this in action:

```rust
use cglue::*;
#[cglue_trait]
pub trait ObjReturn {
    #[wrap_with_obj(InfoPrinter)]
    type ReturnType: InfoPrinter + 'static;

    fn or_1(&self) -> Self::ReturnType;
}

struct InfoBuilder {}

impl ObjReturn for InfoBuilder {
    type ReturnType = Info;

    fn or_1(&self) -> Self::ReturnType {
        Info {
            value: 80
        }
    }
}

let builder = InfoBuilder {};

let obj = trait_obj!(builder as ObjReturn);

let info_printer = obj.or_1();

info_printer.print_info();
```

This also works if the trait were to return a `&Self::ReturnType`, or `&mut Self::ReturnType`.
It is done by storing wrapped return value in an intermediate storage, and then returning
references to there.

However, there is a `SAFETY WARNING`:

Wrapping `&Self::ReturnType` in a function that takes a non-mutable `&self` technically breaks
Rust's safety rules by potentially overwriting data that is already being borrowed as const.
However, in real world a function that takes `&self` and returns `&T` will usually return the
same reference, and it should be alright, but YOU HAVE BEEN WARNED. `TODO: Disallow this?`

The above warning does not apply to `&mut self` functions, because the returned reference is
bound to the same lifetime and can not be re-created while being borrowed.

Generally speaking, you will want to use `wrap_with_obj/wrap_with_group` in `Self::ReturnType`
functions, `wrap_with_obj_mut/wrap_with_group_mut` in `&mut Self::ReturnType` functions, and
`wrap_with_obj_ref/wrap_with_group_ref` in `&Self::ReturnType` functions. It is important to
note that if there is a trait that returns a combination of these types, it is not possible to
use wrapping, because the underlying object types differ. If possible, split up the type to
multiple associated types.

### Working with cbindgen

[cbindgen](https://github.com/eqrion/cbindgen) can be used to generate C and C++ bindings.
There is some important setup needed.

#### Setup

Firstly, create a `cbindgen.toml`, and make sure both cglue, and any crates using cglue have
macro expansion enabled:

```toml
[parse.expand]
crates = ["cglue", "your-crate"]
```

Macro expansion currently requires nightly Rust. Thus, it is then possible to generate bindings
like so:

```sh
rustup run nightly cbindgen --config cbindgen.toml --crate your_crate --output output_header.h
```

You can set C or C++ language mode by appending `-l c` or `-l c++` flag. Alternatively, set it
in the toml:

```toml
language = "C"
```

#### Cleanup C

cbindgen will generate mostly clean output, however, there is one special case it does not
handle well - empty typedefs.

Open the output header, and notice these typedefs:

```c
/**
 * Type definition for temporary return value wrapping storage.
 *
 * The trait does not use return wrapping, thus is a typedef to `PhantomData`.
 *
 * Note that `cbindgen` will generate wrong structures for this type. It is important
 * to go inside the generated headers and fix it - all RetTmp structures without a
 * body should be completely deleted, both as types, and as fields in the
 * groups/objects. If C++11 templates are generated, it is important to define a
 * custom type for CGlueTraitObj that does not have `ret_tmp` defined, and change all
 * type aliases of this trait to use that particular structure.
 */
typedef struct CGlueRetTmpClone CGlueRetTmpClone;
```

Remove all usage of these types. Any variables in the structures with these types should not be
generated as they are intentionally zero-sized. Finally, remove the typedefs.

Automating this step would be very nice, but for now, if editing the headers is not easily
available, enable the `no_empty_retwrap` feature, which will inject 1 byte padding to these
structures.

#### Cleanup C++

Similar error is present in C++ headers, but due to templates, cleanup is slightly different.

Perform the same cleanup as in C headers. But now, you may encounter generic typedefs to
`CGlueTraitObj`. It will be necessary to define a new type without the final value:

```cpp
template<typename T, typename V>
struct CGlueTraitObjSimple {
    T instance;
    const V *vtbl;
};
```

Make all typedefs that use non-existent types use `CGlueTraitObjSimple`.

Finally, there is a wrongly generated `MaybeUninit` typedef. Replace it from this:

```cpp
template<typename T = void>
struct MaybeUninit;
```

To this:

```cpp
template<typename T = void>
using MaybeUninit = T;
```

Other than that, everything should be good to go!

## Limitations

1. Associated type function arguments are not possible, because opaque conversion works
   one-way.

2. Functions that accept an additional `Self` types are not possible for the same reason.

3. Custom generic arguments for cglue traits are not yet supported, but this is to be improved
   upon.

4. There probably are some corner cases when it comes to path imports. If you find any, please
   file an issue report :)

