
# CGlue

[![Crates.io](https://img.shields.io/crates/v/cglue.svg)](https://crates.io/crates/cglue)
[![API Docs](https://docs.rs/cglue/badge.svg)](https://docs.rs/cglue)
[![Build and test](https://github.com/h33p/cglue/actions/workflows/build.yml/badge.svg)](https://github.com/h33p/cglue/actions/workflows/build.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/h33p/cglue/blob/main/LICENSE)
![Rustc 1.45](https://img.shields.io/badge/rustc-1.45+-lightgray.svg)

If all code is glued together, our glue is the safest on the market.

## FFI-safe trait generation, helper structures, and more!

**WARNING: following documentation is currently valid for
[stable 0.1.x series](https://github.com/h33p/cglue/tree/v0.1.3)**

*This is a 0.2 development branch, documentation is to be updated ASAP*

<!-- toc -->
- [Overview](#overview)
- [In-depth look](#in-depth-look)
  - [Safety assumptions](#safety-assumptions)
  - [Name generation](#name-generation)
  - [Generics in groups](#generics-in-groups)
    - [Manully implementing groups](#manually-implementing-groups)
  - [External traits](#external-traits)
  - [Type wrapping](#type-wrapping)
  - [Associated type wrapping](#associated-type-wrapping)
  - [Plugin system](#plugin-system)
  - [Working with cbindgen](#working-with-cbindgen)
    - [Setup](#setup)
    - [Automatic cleanup](#automatic-cleanup)
    - [Cleanup C](#cleanup-c)
    - [Cleanup C++](#cleanup-c-1)
- [Limitations](#limitations)
<!-- /toc -->

## Overview

CGlue offers an easy way to ABI (application binary interface) safety. Just a few annotations and your trait is ready to go!

```rust
use cglue::*;

// One annotation for the trait.
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

fn main() -> () {
    let mut info = Info {
        value: 5
    };

    // Here, the object is fully opaque, and is FFI and ABI safe.
    let obj = trait_obj!(&mut info as InfoPrinter);

    use_info_printer(&obj);
}
```

A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.

This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `trait_obj` macro:

```rust
let obj = InfoPrinterBase::from(&mut info).into_opaque();
```

`cglue_trait` annotation generates a `InfoPrinterVtbl` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.

But that's not all, you can also group traits together!

```rust
use cglue::*;

// Extra trait definitions

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

// Define a trait group.
//
// Here, `InfoPrinter` is mandatory - always required to be implemented,
// whereas `InfoChanger` with `InfoDeleter` are optional traits - a checked
// cast must be performed to access them.
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

This crate relies on the assumption that opaque objects will not be tampered with, that is
vtable functions will not be modified. It is being ensured through encapsulation of fields
from anywhere by using hidden submodules. However, unverifiable users (C libraries) may still
be able to modify the tables. This library assumes they are not malicious and does not
perform any runtime verification. Currently there is no verification of API version mismatches,
but it is in the plans to attempt to integrate with version checking systems available in
[abi\_stable](https://crates.io/crates/abi_stable) crate.

Other than 2 bits in [associated type wrapping](#associated-type-wrapping), this crate should
be safe.

The crate employs a number of `unsafe` traits that get auto-implemented, or traits with unsafe
functions. Their usage inside the code generator should be safe, they are marked in such a way
so that manual implementations can not introduce undefined behaviour.

### Name generation

`#[cglue_trait]` macro for `MyTrait` will generate the following important types:

| Name | Purpose |
--- | ---
| `MyTraitBox` | Typedef for opaque owned CGlue object. Its container is a [`CBox<c_void>`](crate::boxed::CBox) |
| `MyTraitCtxBox<D>` | Typedef for opaque owned CGlue object with an [opaque context](#plugin-system) `D`. Its container is a [`CtxBox<c_void, D>`](crate::boxed::CtxBox) |
| `MyTraitNoCtxBox` | Typedef for opaque owned CGlue object. Its container is a [`CtxBox<c_void, NoContext>`](crate::boxed::CtxBox) |
| `MyTraitArcBox` | Typedef for opaque owned CGlue object with an opaque reference counted context. Its container is a [`CtxBox<c_void, COptArc<c_void>>`](crate::boxed::CtxBox) |
| `MyTraitMut` | Typedef for opaque by-mut-ref CGlue object. Its container is a `&mut c_void`. |
| `MyTraitRef` | Typedef for opaque by-ref (const) CGlue object. Its container is a `&c_void`. |
| `MyTraitAny<T, D>` | Typedef for opaque CGlue object. It can have any compatible container `T` dereferencing to `c_void`, with opaque context `D` |

Only opaque types provide functionality. Non-opaque types can be used as `Into` trait bounds
and are required to type check trait bounds.

These are the generic types needed for bounds checking:

| Name | Purpose |
--- | ---
| `MyTraitBaseBox<F>` | Typedef for generic owned CGlue object. Its container is a [`CBox<F>`](crate::boxed::CBox) |
| `MyTraitBaseCtxBox<F, C>` | Typedef for generic owned CGlue object with [some context](#plugin-system). Its container is a [`CtxBox<F, C>`](crate::boxed::CtxBox) |
| `MyTraitBaseNoCtxBox<F>` | Typedef for generic owned CGlue object with some context. Its container is a [`CtxBox<F, NoContext>`](crate::boxed::CtxBox) |
| `MyTraitBaseArcBox<F, C>` | Typedef for generic owned CGlue object with reference counted context. Its container is a [`CtxBox<F, COptArc<C>`](crate::boxed::CtxBox) |
| `MyTraitBaseMut<F>` | Typedef for generic by-mut-ref CGlue object. Its container is a `&mut F`. |
| `MyTraitBaseRef<F>` | Typedef for generic by-ref (const) CGlue object. Its container is a `&F`. |
| `MyTraitBase<T, F, C, D>` | Base typedef for a CGlue object. It allows for any container type `T`, dereferencing to a concrete type `F`, with context `C` with its opaque version `D`. |

Finally, the following underlying types exist, but do not need to be interacted with in Rust:

| Name | Purpose |
--- | ---
| `MyTraitVtbl<T, F, C, D>` | Table of all functions of the trait. Should be opaque to the user. |
| `MyTraitOpaqueVtbl<D>` | Opaque version of the table. This is the type every object's table will have. |
| `MyTraitRetTmp` | Structure for temporary return values. It should be opaque to the user. |

`cglue_trait_group!` macro for `MyGroup` will generate the following main types:

| Name | Purpose |
--- | ---
| `MyGroupBox` | Typedef for opaque owned CGlue trait group. Its container is a [`CBox<c_void>`](crate::boxed::CBox) |
| `MyGroupCtxBox<D>` | Typedef for opaque owned CGlue trait group with [some context](#plugin-system) `D`. Its container is a [`CtxBox<c_void, D>`](crate::boxed::CtxBox) |
| `MyGroupNoCtxBox` | Typedef for opaque owned CGlue trait group with no true context. Its container is a [`CtxBox<c_void, NoContext>`](crate::boxed::CtxBox) |
| `MyGroupArcBox` | Typedef for opaque owned CGlue trait group with reference counted context. Its container is a [`CtxBox<c_void, COptArc<c_void>>`](crate::boxed::CtxBox) |
| `MyGroupMut` | Typedef for opaque by-mut-ref CGlue trait group. Its container is a `&mut c_void`. |
| `MyGroupRef` | Typedef for opaque by-ref (const) CGlue trait group. Its container is a `&c_void`. |
| `MyGroupAny<T, D>` | Typedef for opaque CGlue trait group. It can have any container. |

Base types are as follows:

| Name | Purpose |
--- | ---
| `MyGroupBaseBox<F>` | Typedef for generic owned CGlue trait group. Its container is a [`CBox<F>`](crate::boxed::CBox) |
| `MyGroupBaseCtxBox<F, C>` | Typedef for generic owned CGlue trait group with [some context](#plugin-system) `C`. Its container is a [`CtxBox<F, C>`](crate::boxed::CtxBox) |
| `MyGroupBaseNoCtxBox<F>` | Typedef for generic owned CGlue trait group with no context. Its container is a [`CtxBox<F, NoContext>`](crate::boxed::CtxBox) |
| `MyGroupBaseArcBox<F, C>` | Typedef for generic owned CGlue trait group with reference counted context. Its container is a [`CtxBox<F, COptArc<C>>`](crate::boxed::CtxBox) |
| `MyGroupBaseMut<F>` | Typedef for generic by-mut-ref CGlue trait group. Its container is a `&mut F`. |
| `MyGroupBaseRef<F>` | Typedef for generic by-ref (const) CGlue trait group. Its container is a `&F`. |
| `MyGroup<T, F, C, D>` | Base definiton of the group. It is not opaque and not usable yet. |

And finally, the filler trait that is required for an object to be grouppable:

| Name | Purpose |
--- | ---
| MyGroupVtableFiller | Trait that allows an object to specify which optional traits are available, through the use of `enable_trait` functions. |

The macro generation will also generate structures for all combinations of optional traits
being used. For more convenient by-macro usage, the names of optional traits inside are sorted
in alphabetical order. If not using macros, check `MyGroup` documentation for underlying
conversion function definitions.

### Generics in groups

Groups are fairly flexible - they are not limited to basic types. They can also contain generic
parameters, associated types, and self returns (this also applies to single-trait objects).

Use of generics in trait groups is rather straightforward, with a couple of tiny nuances.

Define a group with the standard template syntax:

```rust
cglue_trait_group!(GenGroup<T>, Getter<T>, { TA });
```

It is also possible to specify trait bounds:

```rust
cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
```

Or:

```rust
cglue_trait_group!(GenGroup<T> where T: Eq {}, Getter<T>, { TA });
```

Implement the group on a generic type:

```rust
cglue_impl_group!(GA<T: Eq>, GenGroup<T>, { TA });
```

Note that in the above case, `GA<T>` will be grouppable, if, and only if it implements both,
`Getter<T>` and `TA` for `T: Eq`. If `GA` implements different sets of optional traits with
different type parameters, then provide multiple implementations, with specified types. On each
implementation, still add a generic type `T`, but specify its type with an equality somewhere
on the line:

```rust
cglue_impl_group!(GA<T = u64>, GenGroup<T>, {});
cglue_impl_group!(GA<T>, GenGroup<T = usize>, { TA });
```

Here, `GA<u64>` implements only `Getter<T>`, while `GA<usize>` implements both
`Getter<usize>` and `TA`.

Finally, you can also mix the 2, assuming the most general implementation has the most
optional traits defined:

```rust
cglue_impl_group!(GA<T: Eq>, GenGroup<T>, { TA });
cglue_impl_group!(GA<T = u64>, GenGroup<T>, {});
```

#### Manually implementing groups

It is also possible to manually implement the groups by implementing `MyGroupVtableFiller`. Here is what
the above 2 macro invocations expand to:

```rust
impl<
        'cglue_a,
        // Container type
        CGlueT: ContextRef<Context = CGlueC> + Deref<Target = GA<T>>,
        // Base context type, that opaques to `CGlueD`
        CGlueC: 'static + Clone + Send + Sync + Opaquable<OpaqueTarget = CGlueD>,
        // Opaque context type, that has been short-circuit to opaque to self
        CGlueD: 'static + Clone + Send + Sync + Opaquable<OpaqueTarget = CGlueD>,
        // This is the user-provided Eq bound
        T: Eq,
    > GenGroupVtableFiller<'cglue_a, CGlueT, CGlueC, CGlueD, T> for GA<T>
where
    // When we want to enable TA, we must mark that the vtable can be generated
    &'cglue_a TAVtbl<'cglue_a, CGlueT, GA<T>, CGlueC, CGlueD>: 'cglue_a + Default,
{
    fn fill_table(
        table: GenGroupVtables<'cglue_a, CGlueT, GA<T>, CGlueC, CGlueD, T>,
    ) -> GenGroupVtables<'cglue_a, CGlueT, GA<T>, CGlueC, CGlueD, T> {
        table.enable_ta()
    }
}
impl<
        'cglue_a,
        // Container type
        CGlueT: ContextRef<Context = CGlueC> + Deref<Target = GA<u64>>,
        // Base context type, that opaques to `CGlueD`
        CGlueC: 'static + Clone + Send + Sync + Opaquable<OpaqueTarget = CGlueD>,
        // Opaque context type, that has been short-circuit to opaque to self
        CGlueD: 'static + Clone + Send + Sync + Opaquable<OpaqueTarget = CGlueD>,
    > GenGroupVtableFiller<'cglue_a, CGlueT, CGlueC, CGlueD, u64> for GA<u64>
{
    fn fill_table(
        table: GenGroupVtables<'cglue_a, CGlueT, GA<u64>, CGlueC, CGlueD, u64>,
    ) -> GenGroupVtables<'cglue_a, CGlueT, GA<u64>, CGlueC, CGlueD, u64> {
        table
    }
}
```

### External traits

Certain traits may not be available for `#[cglue_trait]` annotation. Thus, there are mechanisms
in place to allow constructing CGlue objects of external traits. The core primitive is
`#[cglue_trait_ext]`. Essentially the user needs to provide a sufficient definition for the
actual trait, like so:

```rust
#[cglue_trait_ext]
pub trait Clone {
    fn clone(&self) -> Self;
}
```

Notice how this trait does not have the `clone_from` function. Having a separate `&Self`
parameter is not supported, but the trait can still be implemented, because `clone_from` is
merely an optional optimization and there already is a blanket implementation for it.

Usage of external traits is the same when constructing single-trait objects. It gets more
complicated when groups are involved. This is how a `MaybeClone` group would be implemented:

```rust
cglue_trait_group!(MaybeClone, { }, { ext::Clone }, {
    pub trait Clone {
        fn clone(&self) -> Self;
    }
});
```

The first change is to use `ext::Clone`. This marks cglue to create external trait glue code.
The second bit is the trait definition. Yes, unfortunately the group needs another definition
of the trait. CGlue does not have the context of the crate, and it needs to know the function
signatures.

This is far from ideal, thus there is an additional mechanism in place - built-in external
traits. It is a store of trait definitions that can be used without providing multiple trait
definitions. With `Clone` being both inside the store, and marked as prelude export, the above
code gets simplified to just this:

```rust
cglue_trait_group!(MaybeClone, { }, { Clone });
```

For traits not in the prelude, they can be accessed through their fully qualified `::ext` path:

```rust
cglue_trait_group!(MaybeAsRef<T>, { }, { ::ext::core::convert::AsRef<T> });
```

Note that `use` imports do not work - a fully qualified path is required.

The trait store is the least complete part of this system. If you encounter missing traits and
wish to use them, please file a pull request with their definitions, and I will be glad to
include them.

### Type wrapping

As for details, commonly used Rust structures are automatically wrapped in a way that works
effectively.

For instance, slices and `str` types get converted to C-compatible slices.

```rust
fn with_slice(&self, slice: &[usize]) {}

// Generated vtable entry:

with_slice: extern "C" fn(&CGlueF, slice: CSlice<usize>),
```

`Option` types that can not have [nullable pointer optimization](https://doc.rust-lang.org/nomicon/ffi.html#the-nullable-pointer-optimization) are wrapped into [COption](crate::option::COption):

```rust
fn non_npo_option(&self, opt: Option<usize>) {}

// Generated vtable entry:

non_npo_option: extern "C" fn(&CGlueF, opt: Option<usize>),
```

`Result` is automatically wrapped into [CResult](crate::result::CResult):

```rust
fn with_cresult(&self) -> Result<usize, usize> {}

// Generated vtable entry:

with_cresult: extern "C" fn(&CGlueF) -> CResult<usize, usize>,
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

In addition, there is quite a bit of type safety being broken when when wrapping associated
types in anonymous lifetime references. It should be okay, but the situation is as follows:

1. Due to no GAT, `CGlueObjRef/Mut<'_>` is being promoted to `CGlueObjRef/Mut<'static>`. This
   should be okay, given it is not possible to clone non-CBox objects, and these objects are
   returned by-reference, not value.

2. Trait bounds are only checked for one lifetime (lifetime of the vtable), and the C function
   is being cast into a HRTB one unsafely. This is because it is not possible to specify the
   HRTB upper bound (`for<'b: 'a>`). It should be okay, since the vtable can be created for the
   vtable's lifetime, the returned reference will not outlive the vtable, and the C function is
   fully type checked otherwise.

However, if there is a glaring issue I am missing, and there is a solution to this unsafety,
please file an issue report.

Generally speaking, you will want to use `wrap_with_obj/wrap_with_group` in `Self::ReturnType`
functions, `wrap_with_obj_mut/wrap_with_group_mut` in `&mut Self::ReturnType` functions, and
`wrap_with_obj_ref/wrap_with_group_ref` in `&Self::ReturnType` functions. It is important to
note that if there is a trait that returns a combination of these types, it is not possible to
use wrapping, because the underlying object types differ. If possible, split up the type to
multiple associated types.

### Plugin system

A full example is available in the repo's `examples` subdirectory.

CGlue currently does not provide an out-of-the box plugin system, but there are primitives in
place for relatively safe trait usage using dynamically loaded libraries. The core primitive is
the `CtxBox` type - this type allows to house a context, such as a `libloading::Library` Arc
which will be automatically cloned into every owned object the trait creates.

```rust
use cglue::prelude::v1::*;

#[cglue_trait]
pub trait PluginRoot {
    // ...
}

impl PluginRoot for () {}

let root = ();
// This could be a `libloading::Library` arc.
let ref_to_count = CArc::from(()).into_opt();
// Merely passing a tuple is enough.
let obj = trait_obj!((root, ref_to_count) as PluginRoot);
// ...
```

Reference counting the Arc allows to safeguard the dynamically loaded library from being
unloaded prematurely.

If `PluginRoot` were to branch out and build new objects that can be dropped after the instance
of `PluginRoot`, for instance an `InfoPrinter` object, the Arc gets moved/cloned into the new
object.

```rust
#[cglue_trait]
pub trait PluginRoot {
    #[wrap_with_obj(InfoPrinter)]
    type PrinterType: InfoPrinter;

    fn get_printer(&self) -> Self::PrinterType;
}

impl PluginRoot for () {
    type PrinterType = Info;

    fn get_printer(&self) -> Self::PrinterType {
        Info { value: 42 }
    }
}

let root = ();
// This could be a `libloading::Library` arc.
let ref_to_count = CArc::from(()).into_opt();
// Construct a `CtxBox` to be more explicit.
let wrapped = CtxBox::from((root, ref_to_count));
let obj = trait_obj!(wrapped as PluginRoot);
let printer = obj.get_printer();
// It is safe to drop obj now:
std::mem::drop(obj);
printer.print_info();
```

Note that this is not foolproof, and there may be situations where returned data could depend
on the library. The most error prone of which are unhandled `Err(E)` conditions, where `E` is
some static str. `main` function could return an error pointing to the memory of the library,
unload it, and then attempt to print it out, resulting in a segfault. If possible, try to use
`IntError` types, and mark the trait with `#[int_result]`, which would prevent this particular
issue from happening.

Another note, any by-ref objects do not have a context attached to them - their context is a
`NoContext` type. If there is a way to construct an owned object out of them, the said object
will not have the original context passed through. If reference counted context is used, this
problem is easy to be type checked, because the returned type will be `MyTraitNoCtxBox`, rather
than `MyTraitArcBox`.

### Working with cbindgen

[cbindgen](https://github.com/eqrion/cbindgen) can be used to generate C and C++ bindings.
There is some important setup needed.

#### Setup

Firstly, create a `cbindgen.toml`, and make sure both cglue, and any crates using cglue are
included and have macro expansion enabled:

```toml
[parse]
parse_deps = true
include = ["cglue", "your-crate"]

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

#### Automatic cleanup

cbindgen will generate mostly clean output, however, there is one special case it does not
handle well - empty typedefs.

[`cglue-bindgen`](https://crates.io/crates/cglue-bindgen) is a cbindgen wrapper that attempts
to automatically clean up the headers. It also adds an ability to automatically invoke nightly
rust with `+nightly` flag. The change is simple - just move all cbindgen arguments after `--`:

```sh
cglue-bindgen +nightly -- --config cbindgen.toml --crate your_crate --output output_header.h
```

If something does not work, below are the steps for manual cleanup in both C and C++ modes.

#### Cleanup C

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
typedef struct CloneRetTmp CloneRetTmp;
```

Remove all usage of these types. Any variables in the structures with these types should not be
generated as they are intentionally zero-sized. Finally, remove the typedefs.

Then, you might notice the following typedef:

```c
/**
 * Describes absence of a context.
 *
 * This context is used for regular `CBox` trait objects as well as by-ref or by-mut objects.
 */
typedef struct NoContext NoContext;
```

Remove it, as well as any usages of it. It works the same way as zero-sized RetTmp variables.

#### Cleanup C++

Similar error is present in C++ headers, but due to templates, cleanup is slightly different.

1. Remove all references to the incomplete types in the generated group structures.

2. Change all incomplete `struct TraitRetTmp;` definitions to `typedef void TraitRetTmp;`

3. Define a specialized type for `CGlueTraitObj` without the final value:

```cpp
template<typename T, typename V, typename S>
struct CGlueTraitObj {
    T instance;
    const V *vtbl;
    S ret_tmp;
};

// Make sure it goes after the original declaration.

template<typename T, typename V>
struct CGlueTraitObj<T, V, void> {
    T instance;
    const V *vtbl;
};
```

Similar specialization should be done for the `CtxBox` type:

```cpp
template<typename T>
struct CtxBox<T, void> {
    CBox<T> inner;
};
```

Finally, there usually is a wrongly generated `MaybeUninit` typedef. Replace it from this:

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

4. There is no runtime validation of ABI differences caused by API changes. It is planned
   for a future release, perhaps integrating with
   [abi\_stable](https://crates.io/crates/abi_stable).

5. There probably are some corner cases when it comes to path imports. If you find any, please
   file an issue report :)
