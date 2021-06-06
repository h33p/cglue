
# CGlue

[![Crates.io](https://img.shields.io/crates/v/cglue.svg)](https://crates.io/crates/cglue)
[![API Docs](https://docs.rs/cglue/badge.svg)](https://docs.rs/cglue)
[![Build and test](https://github.com/h33p/cglue/actions/workflows/build.yml/badge.svg)](https://github.com/h33p/cglue/actions/workflows/build.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/h33p/cglue/blob/main/LICENSE)

If all code is glued together, our glue is the safest on the market.

## FFI-safe trait generation, helper structures, and more!

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

## Associated type wrapping

Associated types can be safely wrapped into custom CGlue objects. Below is a minimal example of
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
same reference, and it should be alright, but YOU HAVE BEEN WARNED.

The above warning does not apply to `&mut self` functions, because the returned reference is
bound to the same lifetime and can not be re-created while being borrowed.

Generally speaking, you will want to use `wrap_with_obj/wrap_with_group` in `Self::ReturnType`
functions, `wrap_with_obj_mut/wrap_with_group_mut` in `&mut Self::ReturnType` functions, and
`wrap_with_obj_ref/wrap_with_group_ref` in `&Self::ReturnType` functions. It is important to
note that if there is a trait that returns a combination of these types, it is not possible to
use wrapping, because the underlying object types differ. If possible, split up the type to
multiple associated types.

## Limitations

1. Associated type function arguments are not possible, because opaque conversion works
   one-way.

2. Functions that accept an additional `Self` types are not possible for the same reason.

3. Custom generic arguments for cglue traits are not yet supported, but this is to be improved
   upon.

4. There probably are some corner cases when it comes to path imports. If you find any, please
   file an issue report :)
