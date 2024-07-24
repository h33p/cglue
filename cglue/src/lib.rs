//!
//! # CGlue
//!
//! [![Crates.io]][crates] [![API Docs]][docs] [![Build and test]][workflows] [![MIT licensed]][license] [![Rustc 1.45]][rust]
//!
//! [Crates.io]: https://img.shields.io/crates/v/cglue.svg
//! [crates]: https://crates.io/crates/cglue
//! [API Docs]: https://docs.rs/cglue/badge.svg
//! [docs]: https://docs.rs/cglue
//! [Build and test]: https://github.com/h33p/cglue/actions/workflows/build.yml/badge.svg
//! [workflows]: https://github.com/h33p/cglue/actions/workflows/build.yml
//! [MIT licensed]: https://img.shields.io/badge/license-MIT-blue.svg
//! [license]: https://github.com/h33p/cglue/blob/main/LICENSE
//! [Rustc 1.45]: https://img.shields.io/badge/rustc-1.45+-lightgray.svg
//! [rust]: https://blog.rust-lang.org/2020/07/16/Rust-1.45.0.html
//!
//! If all code is glued together, our glue is the safest on the market.
//!
//! ## The most complete dynamic trait object implementation, period.
//!
//! <!-- toc -->
//! - [Overview](#overview)
//! - [In-depth look](#in-depth-look)
//!   - [Safety assumptions](#safety-assumptions)
//!   - [Name generation](#name-generation)
//!   - [Generics in groups](#generics-in-groups)
//!     - [Manully implementing groups](#manually-implementing-groups)
//!   - [External traits](#external-traits)
//!   - [Type wrapping](#type-wrapping)
//!   - [Associated type wrapping](#associated-type-wrapping)
//!   - [Generic associated types](#generic-associated-types)
//!   - [Plugin system](#plugin-system)
//!   - [Working with cbindgen](#working-with-cbindgen)
//!     - [Setup](#setup)
//!     - [cglue-bindgen](#cglue-bindgen)
//! - [Limitations](#limitations)
//!   - [Unstable feature](#unstable-feature)
//! - [Projects using CGlue](#projects-using-cglue)
//! - [Changelog](#changelog)
//! <!-- /toc -->
//!
//! ## Overview
//!
//! CGlue exposes `dyn Trait` in FFI-safe manner. It bridges Rust traits between C and other
//! languages. It aims to be seamless to integrate - just add a few annotations around your traits,
//! and they should be good to go!
//!
//! ```rust
//! use cglue::*;
//!
//! // One annotation for the trait.
//! #[cglue_trait]
//! pub trait InfoPrinter {
//!     type Mark;
//!     fn print_info(&self, mark: Self::Mark);
//! }
//!
//! struct Info {
//!     value: usize
//! }
//!
//! impl InfoPrinter for Info {
//!     type Mark = u8;
//!
//!     fn print_info(&self, mark: Self::Mark) {
//!         println!("{} - info struct: {}", mark, self.value);
//!     }
//! }
//!
//! fn use_info_printer<T: InfoPrinter>(printer: &T, mark: T::Mark) {
//!     println!("Printing info:");
//!     printer.print_info(mark);
//! }
//!
//! fn main() -> () {
//!     let mut info = Info {
//!         value: 5
//!     };
//!
//!     // Here, the object is fully opaque, and is FFI and ABI safe.
//!     let obj = trait_obj!(&mut info as InfoPrinter);
//!
//!     use_info_printer(&obj, 42);
//! }
//! ```
//!
//! Rust does not guarantee your code will work with
//! neither [2 different compiler versions clashing](https://pastebin.com/raw/un1TbJCe), nor [any other minor changes](https://github.com/rust-lang/compiler-team/issues/457),
//! CGlue glues it all together in a way that works.
//!
//! This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table.
//!
//! `cglue_trait` annotation generates a `InfoPrinterVtbl` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.
//!
//! But that's not all, you can also group traits together!
//!
//! ```
//! use cglue::*;
//!
//! # // Previous definitions
//! # #[cglue_trait]
//! # pub trait InfoPrinter {
//! #     fn print_info(&self);
//! # }
//! # struct Info {
//! #     value: usize
//! # }
//! # impl InfoPrinter for Info {
//! #     fn print_info(&self) {
//! #         println!("Info struct: {}", self.value);
//! #     }
//! # }
//! // Extra trait definitions
//!
//! #[cglue_trait]
//! pub trait InfoChanger {
//!     fn change_info(&mut self, new_val: usize);
//! }
//!
//! impl InfoChanger for Info {
//!     fn change_info(&mut self, new_val: usize) {
//!         self.value = new_val;
//!     }
//! }
//!
//! #[cglue_trait]
//! pub trait InfoDeleter {
//!     fn delete_info(&mut self);
//! }
//!
//! // Define a trait group.
//! //
//! // Here, `InfoPrinter` is mandatory - always required to be implemented,
//! // whereas `InfoChanger` with `InfoDeleter` are optional traits - a checked
//! // cast must be performed to access them.
//! cglue_trait_group!(InfoGroup, InfoPrinter, { InfoChanger, InfoDeleter });
//!
//! // Implement the group for `Info` structure, defining
//! // only that `InfoChanger` is optionally implemented.
//! // This is not required if `unstable` feature is being used!
//! # #[cfg(not(feature = "unstable"))]
//! cglue_impl_group!(Info, InfoGroup, InfoChanger);
//!
//! # fn main() -> () {
//! let mut info = Info { value: 5 };
//!
//! let mut obj = group_obj!(info as InfoGroup);
//!
//! // Object does not implement `InfoDeleter`
//! assert!(as_ref!(&obj impl InfoDeleter).is_none());
//!
//! change_info(&mut cast!(obj impl InfoChanger).unwrap(), 20);
//! # }
//!
//! fn change_info(change: &mut (impl InfoPrinter + InfoChanger), new_val: usize) {
//!     println!("Old info:");
//!     change.print_info();
//!     change.change_info(new_val);
//!     println!("New info:");
//!     change.print_info();
//! }
//! ```
//!
//! And there is much more! Here are some highlights:
//!
//! 1. Ability to use self-consuming trait functions.
//!
//! 2. Some standard library traits are exposed (`Clone`).
//!
//! 3. Ability to wrap associated trait types into new CGlue trait objects and groups.
//!
//! 4. The above ability also works with mutable, and const reference associated type returns[*](#associated-type-wrapping).
//!
//! 5. Generic traits and their groups.
//!
//! 6. [Library reference counting](#plugin-system).
//!
//! 7. Optional runtime ABI/API validation with [abi\_stable](https://crates.io/crates/abi_stable) (enable `layout_checks` feature).
//!
//! ## In-depth look
//!
//! ### Safety assumptions
//!
//! This crate relies on the assumption that opaque objects will not be tampered with, that is
//! vtable functions will not be modified. It is being ensured through encapsulation of fields
//! from anywhere by using hidden submodules. However, unverifiable users (C libraries) may still
//! be able to modify the tables. This library assumes they are not malicious and does not
//! perform any runtime verification. API version mismatch checking with
//! [abi\_stable](https://crates.io/crates/abi_stable) is an opt-in feature (requires rustc 1.46+).
//!
//! Other than 2 bits in [associated type wrapping](#associated-type-wrapping), this crate should
//! be safe.
//!
//! The crate employs a number of `unsafe` traits that get auto-implemented, or traits with unsafe
//! functions. Their usage inside the code generator should be safe, they are marked in such a way
//! so that manual implementations can not introduce undefined behaviour.
//!
//! ### Name generation
//!
//! `#[cglue_trait]` macro for `MyTrait` will generate the following important types:
//!
//! | Name | Purpose | Instance type | [Context](#plugin-system) |
//! --- | --- | --- | ----
//! | `MyTraitBox` | Regular owned CGlue object. | [`CBox<c_void>`](crate::boxed::CBox) | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitCtxBox<Ctx>` | Owned CGlue object with a [context](#plugin-system). | [`CBox<c_void>`](crate::boxed::CBox) | `Ctx` |
//! | `MyTraitArcBox` | Owned CGlue object with a reference counted context. | [`CBox<c_void>`](crate::boxed::CBox) | [`CArc<c_void>`](crate::arc::CArc) |
//! | `MyTraitMut` | By-mut-ref CGlue object. | `&mut c_void`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitCtxMut<Ctx>` | By-mut-ref CGlue object with a context. | `&mut c_void`. | `Ctx` |
//! | `MyTraitArcMut` | By-mut-ref CGlue object with a reference counted context. | `&mut c_void`. | [`CArc<c_void>`](crate::arc::CArc) |
//! | `MyTraitRef` | By-ref (const) CGlue object. | `&c_void`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitCtxRef<Ctx>` | By-ref (const) CGlue object with a context. | `&c_void`. | `Ctx` |
//! | `MyTraitArcRef` | By-ref (const) CGlue object with a reference counted context. | `&c_void`. | [`CArc<c_void>`](crate::arc::CArc) |
//!
//! Only opaque types provide functionality. Non-opaque types can be used as `Into` trait bounds
//! and are required to type check trait bounds.
//!
//! These are the generic types needed for bounds checking:
//!
//! | Name | Purpose | Instance type | Context |
//! --- | --- | --- | ---
//! | `MyTraitBaseBox<T>` | Base owned CGlue object. | [`CBox<T>`](crate::boxed::CBox) | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitBaseCtxBox<T, Ctx>` | Base owned CGlue object with [some context](#plugin-system). | [`CBox<T>`](crate::boxed::CBox) | `Ctx` |
//! | `MyTraitBaseArcBox<T, Ctx>` | Base owned CGlue object with reference counted context. | [`CBox<T>`](crate::boxed::CBox) | [`CArc<Ctx>`](crate::arc::CArc) |
//! | `MyTraitBaseMut<T>` | Base by-mut-ref CGlue object. | `&mut T`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitBaseRef<T>` | Typedef for generic by-ref (const) CGlue object. | `&T`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyTraitBase<Inst, Ctx>` | Base (non-opaque) CGlue object. It can have any compatible instance and context | `Inst` | `Ctx` |
//!
//! Finally, the following underlying types exist, but do not need to be interacted with in Rust:
//!
//! | Name | Purpose |
//! --- | ---
//! | `MyTraitVtbl<C>` | Table of all functions of the trait. Should be opaque to the user. |
//! | `MyTraitRetTmp<Ctx>` | Structure for temporary return values. It should be opaque to the user. |
//!
//! Instead, every opaque CGlue object implements `MyTraitOpaqueObj` trait, which contains the type
//! of the vtable.
//!
//! `cglue_trait_group!` macro for `MyGroup` will generate the following main types:
//!
//! | Name | Purpose | Instance type | Context |
//! --- | --- | --- | ---
//! | `MyGroupBox` | Owned CGlue trait group. | [`CBox<c_void>`](crate::boxed::CBox) | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyGroupCtxBox<Ctx>` | Owned CGlue trait group with [some context](#plugin-system). | [`CBox<c_void>`](crate::boxed::CBox) | `Ctx` |
//! | `MyGroupArcBox` | Typedef for opaque owned CGlue trait group with reference counted context. | [`CBox<c_void>`](crate::boxed::CBox) | [`CArc<c_void>`](crate::arc::CArc) |
//! | `MyGroupMut` | Typedef for opaque by-mut-ref CGlue trait group. | `&mut c_void`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyGroupCtxMut<Ctx>` | Typedef for opaque by-mut-ref CGlue trait group with a custom context. | `&mut c_void`. | `Ctx` |
//! | `MyGroupArcMut` | Typedef for opaque by-mut-ref CGlue trait group with a reference counted context. | `&mut c_void`. | [`CArc<c_void>`](crate::arc::CArc) |
//! | `MyGroupRef` | Typedef for opaque by-ref (const) CGlue trait group. | `&c_void`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyGroupCtxRef<Ctx>` | Typedef for opaque by-ref (const) CGlue trait group with a custom context. | `&c_void`. | `Ctx` |
//! | `MyGroupArcRef` | Typedef for opaque by-ref (const) CGlue trait group with a reference counted context. | `&c_void`. | [`CArc<c_void>`](crate::arc::CArc) |
//!
//! Base types are as follows:
//!
//! | Name | Purpose | Instance type | Context |
//! --- | --- | --- | ---
//! | `MyGroupBaseBox<T>` | Base owned CGlue trait group. Its container is a [`CBox<T>`](crate::boxed::CBox) |
//! | `MyGroupBaseCtxBox<T, Ctx>` | Base owned CGlue trait group with [some context](#plugin-system). | [`CBox<T>`](crate::boxed::CBox) | `Ctx` |
//! | `MyGroupBaseArcBox<T, Ctx>` | Base owned CGlue trait group with reference counted context. | [`CBox<T>`](crate::boxed::CBox) | [`CArc<Ctx>`](crate::arc::CArc) |
//! | `MyGroupBaseMut<T>` | Base by-mut-ref CGlue trait group. | `&mut T`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyGroupBaseCtxMut<T, Ctx>` | Base by-mut-ref CGlue trait group with a context. | `&mut T`. | `Ctx` |
//! | `MyGroupBaseArcMut<T, Ctx>` | Base by-mut-ref CGlue trait group with a reference counted context. | `&mut T`. | [`CArc<Ctx>`](crate::arc::CArc) |
//! | `MyGroupBaseRef<T>` | Base by-ref (const) CGlue trait group. | `&T`. | [`NoContext`](crate::trait_group::NoContext) |
//! | `MyGroupBaseCtxRef<T, Ctx>` | Base by-ref (const) CGlue trait group with a context. | `&T`. | `Ctx` |
//! | `MyGroupBaseArcRef<T, Ctx>` | Base by-ref (const) CGlue trait group with a reference counted context. | `&T`. | [`CArc<Ctx>`](crate::arc::CArc) |
//! | `MyGroup<Inst, Ctx>` | Base definiton of the group. It needs to be manually made opaque. | `Inst` | `Ctx` |
//!
//! Container type (opaque to Rust users) that is placed within the group:
//!
//! | Name | Purpose |
//! --- | ---
//! | `MyGroupContainer<Inst, Ctx>` | Stores temporary return storage. Vtables are built for this type.
//!
//! And finally, the filler trait that is required for an object to be grouppable:
//!
//! | Name | Purpose |
//! --- | ---
//! | `MyGroupVtableFiller` | Trait that allows an object to specify which optional traits are available, through the use of `enable_trait` functions. |
//!
//! The macro generation will also generate structures for all combinations of optional traits
//! being used. For more convenient by-macro usage, the names of optional traits inside are sorted
//! in alphabetical order. If not using macros, check `MyGroup` documentation for underlying
//! conversion function definitions.
//!
//! ### Generics in groups
//!
//! Groups are fairly flexible - they are not limited to basic types. They can also contain generic
//! parameters, associated types, and self returns (this also applies to single-trait objects).
//!
//! Use of generics in trait groups is rather straightforward, with a couple of tiny nuances.
//!
//! Define a group with the standard template syntax:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! cglue_trait_group!(GenGroup<T>, Getter<T>, { TA });
//! # fn main() {}
//! ```
//!
//! It is also possible to specify trait bounds:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
//! # fn main() {}
//! ```
//!
//! Or:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! cglue_trait_group!(GenGroup<T> where T: Eq {}, Getter<T>, { TA });
//! # fn main() {}
//! ```
//!
//! Implement the group on a generic type:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! # cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
//! cglue_impl_group!(GA<T: Eq>, GenGroup<T>, { TA });
//! # fn main() {}
//! ```
//!
//! Note that in the above case, `GA<T>` will be grouppable, if, and only if it implements both,
//! `Getter<T>` and `TA` for `T: Eq`. If `GA` implements different sets of optional traits with
//! different type parameters, then provide multiple implementations, with specified types. On each
//! implementation, still add a generic type `T`, but specify its type with an equality somewhere
//! on the line:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! # cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
//! cglue_impl_group!(GA<T = u64>, GenGroup<T>, {});
//! cglue_impl_group!(GA<T>, GenGroup<T = usize>, { TA });
//! # fn main() {}
//! ```
//!
//! Here, `GA<u64>` implements only `Getter<T>`, while `GA<usize>` implements both
//! `Getter<usize>` and `TA`.
//!
//! Finally, you can also mix the 2, assuming the most general implementation has the most
//! optional traits defined:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! # cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
//! cglue_impl_group!(GA<T: Eq>, GenGroup<T>, { TA });
//! cglue_impl_group!(GA<T = u64>, GenGroup<T>, {});
//! # fn main() {}
//! ```
//!
//! #### Manually implementing groups
//!
//! NOTE: This is not supported if [`unstable`](#unstable-feature) feature is enabled. Instead, you
//! have to do nothing!
//!
//! It is also possible to manually implement the groups by implementing `MyGroupVtableFiller`. Here is what
//! the above 2 macro invocations expand to:
//!
//! ```
//! # use cglue::*;
//! # #[cglue_trait]
//! # pub trait TA {
//! #     extern "C" fn ta_1(&self) -> usize;
//! # }
//! # #[cglue_trait]
//! # pub trait Getter<T> {
//! #     fn get_val(&self) -> &T;
//! # }
//! # pub struct GA<T> {
//! #     val: T
//! # }
//! # impl<T> Getter<T> for GA<T> {
//! #     fn get_val(&self) -> &T {
//! #         &self.val
//! #     }
//! # }
//! # impl TA for GA<usize> {
//! #     extern "C" fn ta_1(&self) -> usize {
//! #         self.val
//! #     }
//! # }
//! # cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
//! # use core::ops::Deref;
//! # use cglue::trait_group::{Opaquable};
//! # #[cfg(not(feature = "unstable"))]
//! impl<
//!         'cglue_a,
//!         CGlueInst: ::core::ops::Deref<Target = GA<T>>,
//!         CGlueCtx: cglue::trait_group::ContextBounds,
//!         T: Eq,
//!     > GenGroupVtableFiller<'cglue_a, CGlueInst, CGlueCtx, T> for GA<T>
//! where
//!     Self: TA,
//!     &'cglue_a TAVtbl<'cglue_a, GenGroupContainer<CGlueInst, CGlueCtx, T>,
//!     >:
//!         'cglue_a + Default,
//!     T: cglue::trait_group::GenericTypeBounds,
//! {
//!     fn fill_table(
//!         table: GenGroupVtables<'cglue_a, CGlueInst, CGlueCtx, T>,
//!     ) -> GenGroupVtables<'cglue_a, CGlueInst, CGlueCtx, T> {
//!         table.enable_ta()
//!     }
//! }
//! # #[cfg(not(feature = "unstable"))]
//! impl<
//!         'cglue_a,
//!         CGlueInst: ::core::ops::Deref<Target = GA<u64>>,
//!         CGlueCtx: cglue::trait_group::ContextBounds,
//!     > GenGroupVtableFiller<'cglue_a, CGlueInst, CGlueCtx, u64> for GA<u64>
//! {
//!     fn fill_table(
//!         table: GenGroupVtables<'cglue_a, CGlueInst, CGlueCtx, u64>,
//!     ) -> GenGroupVtables<'cglue_a, CGlueInst, CGlueCtx, u64> {
//!         table
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! ### External traits
//!
//! Certain traits may not be available for `#[cglue_trait]` annotation. Thus, there are mechanisms
//! in place to allow constructing CGlue objects of external traits. The core primitive is
//! `#[cglue_trait_ext]`. Essentially the user needs to provide a sufficient definition for the
//! actual trait, like so:
//!
//! ```ignore
//! # use cglue::*;
//! #[cglue_trait_ext]
//! pub trait Clone {
//!     fn clone(&self) -> Self;
//! }
//! # fn main() {}
//! ```
//!
//! Notice how this trait does not have the `clone_from` function. Having a separate `&Self`
//! parameter is not supported, but the trait can still be implemented, because `clone_from` is
//! merely an optional optimization and there already is a blanket implementation for it.
//!
//! Usage of external traits is the same when constructing single-trait objects. It gets more
//! complicated when groups are involved. This is how a `MaybeClone` group would be implemented:
//!
//! ```ignore
//! # use cglue::*;
//! # #[cglue_trait_ext]
//! # pub trait Clone {
//! #     fn clone(&self) -> Self;
//! # }
//! cglue_trait_group!(MaybeClone, { }, { ext::Clone }, {
//!     pub trait Clone {
//!         fn clone(&self) -> Self;
//!     }
//! });
//! # fn main() {}
//! ```
//!
//! The first change is to use `ext::Clone`. This marks cglue to create external trait glue code.
//! The second bit is the trait definition. Yes, unfortunately the group needs another definition
//! of the trait. CGlue does not have the context of the crate, and it needs to know the function
//! signatures.
//!
//! This is far from ideal, thus there is an additional mechanism in place - built-in external
//! traits. It is a store of trait definitions that can be used without providing multiple trait
//! definitions. With `Clone` being both inside the store, and marked as prelude export, the above
//! code gets simplified to just this:
//!
//! ```
//! # use cglue::*;
//! cglue_trait_group!(MaybeClone, { }, { Clone });
//! # fn main() {}
//! ```
//!
//! For traits not in the prelude, they can be accessed through their fully qualified `::ext` path:
//!
//! ```
//! # use cglue::*;
//! cglue_trait_group!(MaybeAsRef<T>, { }, { ::ext::core::convert::AsRef<T> });
//! # fn main() {}
//! ```
//!
//! Note that `use` imports do not work - a fully qualified path is required.
//!
//! The trait store is the least complete part of this system. If you encounter missing traits and
//! wish to use them, please file a pull request with their definitions, and I will be glad to
//! include them.
//!
//! ### Type wrapping
//!
//! As for details, commonly used Rust structures are automatically wrapped in a way that works
//! effectively.
//!
//! For instance, slices and `str` types get converted to C-compatible slices.
//!
//! ```ignore
//! fn with_slice(&self, slice: &[usize]) {}
//!
//! // Generated vtable entry:
//!
//! with_slice: extern "C" fn(&CGlueC, slice: CSlice<usize>),
//! ```
//!
//! `Option` types that can not have [nullable pointer optimization](https://doc.rust-lang.org/nomicon/ffi.html#the-nullable-pointer-optimization) are wrapped into [COption](crate::option::COption):
//!
//! ```ignore
//! fn non_npo_option(&self, opt: Option<usize>) {}
//!
//! // Generated vtable entry:
//!
//! non_npo_option: extern "C" fn(&CGlueC, opt: Option<usize>),
//! ```
//!
//! `Result` is automatically wrapped into [CResult](crate::result::CResult):
//!
//! ```ignore
//! fn with_cresult(&self) -> Result<usize, usize> {}
//!
//! // Generated vtable entry:
//!
//! with_cresult: extern "C" fn(&CGlueC) -> CResult<usize, usize>,
//! ```
//!
//! `Result` with [IntError](crate::result::IntError) type can return an integer code with `Ok` value written to a variable:
//!
//! ```ignore
//! #[int_result]
//! fn with_int_result(&self) -> Result<usize> {}
//!
//! // Generated vtable entry:
//!
//! with_int_result: extern "C" fn(&CGlueC, ok_out: &mut MaybeUninit<usize>) -> i32,
//! ```
//!
//! All wrapping and conversion is handled transparently behind the scenes, with user's control.
//!
//! ### Associated type wrapping
//!
//! Associated types can be wrapped into custom CGlue objects. Below is a minimal example of
//! this in action:
//!
//! ```
//! use cglue::*;
//! # // Previous definitions
//! # #[cglue_trait]
//! # pub trait InfoPrinter {
//! #     fn print_info(&self);
//! # }
//! # struct Info {
//! #     value: usize
//! # }
//! # impl InfoPrinter for Info {
//! #     fn print_info(&self) {
//! #         println!("Info struct: {}", self.value);
//! #     }
//! # }
//! #[cglue_trait]
//! pub trait ObjReturn {
//!     #[wrap_with_obj(InfoPrinter)]
//!     type ReturnType: InfoPrinter + 'static;
//!
//!     fn or_1(&self) -> Self::ReturnType;
//! }
//!
//! struct InfoBuilder {}
//!
//! impl ObjReturn for InfoBuilder {
//!     type ReturnType = Info;
//!
//!     fn or_1(&self) -> Self::ReturnType {
//!         Info {
//!             value: 80
//!         }
//!     }
//! }
//!
//! # fn main() {
//! let builder = InfoBuilder {};
//!
//! let obj = trait_obj!(builder as ObjReturn);
//!
//! let info_printer = obj.or_1();
//!
//! info_printer.print_info();
//! # }
//! ```
//!
//! This also works if the trait were to return a `&Self::ReturnType`, or `&mut Self::ReturnType`.
//! It is done by storing wrapped return value in an intermediate storage, and then returning
//! references to there.
//!
//! However, there is a `SAFETY WARNING`:
//!
//! Wrapping `&Self::ReturnType` in a function that takes a non-mutable `&self` technically breaks
//! Rust's safety rules by potentially overwriting data that is already being borrowed as const.
//! However, in real world a function that takes `&self` and returns `&T` will usually return the
//! same reference, and it should be alright, but YOU HAVE BEEN WARNED. `TODO: Disallow this?`
//!
//! The above warning does not apply to `&mut self` functions, because the returned reference is
//! bound to the same lifetime and can not be re-created while being borrowed.
//!
//! In addition, there is quite a bit of type safety being broken when when wrapping associated
//! types in anonymous lifetime references. It should be okay, but the situation is as follows:
//!
//! 1. Due to no GAT, `CGlueObjRef/Mut<'_>` is being promoted to `CGlueObjRef/Mut<'static>`. This
//!    should be okay, given it is not possible to clone non-CBox objects, and these objects are
//!    returned by-reference, not value (see GATs section for how to avoid this).
//!
//! 2. Trait bounds are only checked for one lifetime (lifetime of the vtable), and the C function
//!    is being cast into a HRTB one unsafely. This is because it is not possible to specify the
//!    HRTB upper bound (`for<'b: 'a>`). It should be okay, since the vtable can be created for the
//!    vtable's lifetime, the returned reference will not outlive the vtable, and the C function is
//!    fully type checked otherwise.
//!
//! However, if there is a glaring issue I am missing, and there is a solution to this unsafety,
//! please file an issue report.
//!
//! Generally speaking, you will want to use `wrap_with_obj/wrap_with_group` in `Self::ReturnType`
//! functions, `wrap_with_obj_mut/wrap_with_group_mut` in `&mut Self::ReturnType` functions, and
//! `wrap_with_obj_ref/wrap_with_group_ref` in `&Self::ReturnType` functions. It is important to
//! note that if there is a trait that returns a combination of these types, it is not possible to
//! use wrapping, because the underlying object types differ. If possible, split up the type to
//! multiple associated types.
//!
//! ### Generic associated types
//!
//! CGlue has limited support for GATs! More specifically, single lifetime GATs are supported,
//! which allows one to implement a form of `LendingIterator`:
//!
//! ```
//! # #[cfg(gats_on_stable)]
//! # mod gats {
//! use cglue::*;
//! # // Previous definitions
//! # #[cglue_trait]
//! # pub trait InfoPrinter {
//! #     fn print_info(&self);
//! # }
//! # struct Info {
//! #     value: usize
//! # }
//! # impl InfoPrinter for Info {
//! #     fn print_info(&self) {
//! #         println!("Info struct: {}", self.value);
//! #     }
//! # }
//! #[cglue_trait]
//! pub trait LendingPrinter {
//!     #[wrap_with_obj(InfoPrinter)]
//!     type Printer<'a>: InfoPrinter + 'a where Self: 'a;
//!
//!     fn borrow_printer<'a>(&'a mut self) -> Self::Printer<'a>;
//! }
//!
//! impl<'a> InfoPrinter for &'a mut Info {
//!     fn print_info(&self) {
//!         (**self).print_info();
//!     }
//! }
//!
//! struct InfoStore {
//!     info: Info,
//! }
//!
//! impl LendingPrinter for InfoStore {
//!     type Printer<'a> = &'a mut Info;
//!
//!     fn borrow_printer(&mut self) -> Self::Printer<'_> {
//!         &mut self.info
//!     }
//! }
//!
//! # fn main() {
//! let builder = InfoStore { info: Info { value: 50 } };
//!
//! let mut obj = trait_obj!(builder as LendingPrinter);
//!
//! let info_printer = obj.borrow_printer();
//!
//! info_printer.print_info();
//! # }
//! # }
//! ```
//!
//! ### Plugin system
//!
//! A full example is available in the repo's `examples` subdirectory.
//!
//! CGlue currently does not provide an out-of-the box plugin system, but there are primitives in
//! place for relatively safe trait usage using dynamically loaded libraries. The core primitive is
//! a cloneable context, such as a libloading::Library` Arc, which will keep the library opened
//! until all of the CGlue objects are dropped.
//!
//! ```
//! use cglue::prelude::v1::*;
//!
//! #[cglue_trait]
//! pub trait PluginRoot {
//!     // ...
//! }
//!
//! impl PluginRoot for () {}
//!
//! # fn main() -> () {
//! let root = ();
//! // This could be a `libloading::Library` arc.
//! let ref_to_count = CArc::from(());
//! // Merely passing a tuple is enough.
//! let obj = trait_obj!((root, ref_to_count) as PluginRoot);
//! // ...
//! # }
//! ```
//!
//! Reference counting the Arc allows to safeguard the dynamically loaded library from being
//! unloaded prematurely.
//!
//! If `PluginRoot` were to branch out and build new objects that can be dropped after the instance
//! of `PluginRoot`, for instance an `InfoPrinter` object, the Arc gets moved/cloned into the new
//! object.
//!
//! ```
//! # use cglue::prelude::v1::*;
//! # use std::sync::Arc;
//! # #[cglue_trait]
//! # pub trait InfoPrinter {
//! #     fn print_info(&self);
//! # }
//! # pub struct Info {
//! #     value: usize
//! # }
//! # impl InfoPrinter for Info {
//! #     fn print_info(&self) {
//! #         println!("Info struct: {}", self.value);
//! #     }
//! # }
//! #[cglue_trait]
//! pub trait PluginRoot {
//!     #[wrap_with_obj(InfoPrinter)]
//!     type PrinterType: InfoPrinter;
//!
//!     fn get_printer(&self) -> Self::PrinterType;
//! }
//!
//! impl PluginRoot for () {
//!     type PrinterType = Info;
//!
//!     fn get_printer(&self) -> Self::PrinterType {
//!         Info { value: 42 }
//!     }
//! }
//!
//! # fn main() -> () {
//! let root = ();
//! // This could be a `libloading::Library` arc.
//! let ref_to_count = CArc::from(());
//! let obj = trait_obj!((root, ref_to_count) as PluginRoot);
//! let printer = obj.get_printer();
//! // It is safe to drop obj now:
//! std::mem::drop(obj);
//! printer.print_info();
//! # }
//! ```
//!
//! Note that this is not foolproof, and there may be situations where returned data could depend
//! on the library. The most error prone of which are unhandled `Err(E)` conditions, where `E` is
//! some static str. `main` function could return an error pointing to the memory of the library,
//! unload it, and then attempt to print it out, resulting in a segfault. If possible, try to use
//! `IntError` types, and mark the trait with `#[int_result]`, which would prevent this particular
//! issue from happening.
//!
//! ### Working with cbindgen
//!
//! [cbindgen](https://github.com/eqrion/cbindgen) can be used to generate C and C++ bindings.
//! There is some important setup needed.
//!
//! In addition, [`cglue-bindgen`](https://crates.io/crates/cglue-bindgen) provides additional
//! helper method generation, making working with CGlue from C/C++ much more convenient.
//!
//! #### Setup
//!
//! Firstly, create a `cbindgen.toml`, and make sure both cglue, and any crates using cglue are
//! included and have macro expansion enabled:
//!
//! ```toml
//! [parse]
//! parse_deps = true
//! include = ["cglue", "your-crate"]
//!
//! [parse.expand]
//! crates = ["cglue", "your-crate"]
//! ```
//!
//! Macro expansion currently requires nightly Rust. Thus, it is then possible to generate bindings
//! like so:
//!
//! ```sh
//! rustup run nightly cbindgen --config cbindgen.toml --crate your_crate --output output_header.h
//! ```
//!
//! You can set C or C++ language mode by appending `-l c` or `-l c++` flag. Alternatively, set it
//! in the toml:
//!
//! ```toml
//! language = "C"
//! ```
//!
//! Export any shortened typedefs that are not used by any of the extern C functions:
//!
//! ```toml
//! [export]
//! include = ["FeaturesGroupArcBox", "PluginInnerRef", "PluginInnerMut"]
//! ```
//!
//! #### cglue-bindgen
//!
//! [`cglue-bindgen`](https://crates.io/crates/cglue-bindgen) is a cbindgen wrapper that attempts
//! to automatically clean up the headers. It also adds an ability to automatically invoke nightly
//! rust with `+nightly` flag, and also generates vtable wrappers for simpler usage. The change is
//! simple - just move all cbindgen arguments after `--`:
//!
//! ```sh
//! cglue-bindgen +nightly -- --config cbindgen.toml --crate your_crate --output output_header.h
//! ```
//!
//! This wrapper is probably the most fragile part of CGlue - if something does not work, please
//! open up an issue report. In the future, we will aim to integrate CGlue directly with cbindgen.
//!
//! ## Limitations
//!
//! 1. Associated type function arguments are not possible, because opaque conversion works
//!    one-way.
//!
//! 2. Functions that accept an additional `Self` types are not possible for the same reason.
//!
//! 3. Custom generic arguments for cglue traits are not yet supported, but this is to be improved
//!    upon.
//!
//! 4. There probably are some corner cases when it comes to path imports. If you find any, please
//!    file an issue report :)
//!
//! ### Unstable feature
//!
//! `cglue_impl_group` may force you into making conservative optional trait choices, because it is
//! currently not possible to specialize these cases with stable Rust features. But this is not
//! always desirable. You can solve this, by enabling `unstable` feature.
//!
//! This feature makes `cglue_impl_group` a no-op, and automatically enables the widest set of
//! traits for the given object.
//!
//! To use it you need to either:
//!
//! - `nightly` Rust compiler.
//!
//! - Set `RUSTC_BOOTSTRAP=try_default` environment variable when building.
//!
//! Do note, however, that **Rust's stability guarantees get invalidated** by either of these 2
//! options.
//!
//! ## Projects using CGlue
//!
//! * [memflow](https://github.com/memflow/memflow)
//!
//! If you want your project to be added to the list, please open an issue report :)
//!
//! ## Changelog
//!
//! It is available in [CHANGELOG.md](https://github.com/h33p/cglue/blob/main/CHANGELOG.md) file.
//!

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate no_std_compat as std;

#[cfg(feature = "futures")]
extern crate _futures as futures;

pub mod arc;
pub mod boxed;
pub mod callback;
pub mod forward;
pub mod from2;
pub mod iter;
pub mod option;
pub mod repr_cstring;
pub mod result;
pub mod slice;
pub mod trait_group;
pub mod tuple;
pub mod vec;

#[cfg(feature = "task")]
#[cfg_attr(docsrs, doc(cfg(feature = "task")))]
pub mod task;

pub use ::cglue_macro::{
    as_mut, as_ref, cast, cglue_forward, cglue_forward_ext, cglue_impl_group, cglue_trait,
    cglue_trait_ext, cglue_trait_group, custom_impl, group_obj, int_result, into, no_int_result,
    return_wrap, skip_func, trait_obj, vtbl_only, wrap_with, wrap_with_group, wrap_with_group_mut,
    wrap_with_group_ref, wrap_with_obj, wrap_with_obj_mut, wrap_with_obj_ref,
};

#[cfg(feature = "unstable")]
pub use try_default::TryDefault;

pub mod ext {
    //! # Built-in external traits.
    //!
    //! All of the traits implemented here are usable in trait groups and objects.
    cglue_macro::cglue_builtin_ext_traits!();
}

pub mod prelude {
    //! # Import most commonly used types.

    pub mod v1 {
        pub use crate::{
            arc::{CArc, CArcSome},
            boxed::{CBox, CSliceBox},
            callback::{Callback, Callbackable, FeedCallback, FromExtend, OpaqueCallback},
            forward::{Forward, ForwardMut, Fwd},
            iter::CIterator,
            option::COption,
            repr_cstring::{ReprCStr, ReprCString},
            result::{CResult, IntError, IntResult},
            slice::{CSliceMut, CSliceRef},
            trait_group::Opaquable,
            tuple::*,
            vec::CVec,
            *,
        };

        #[cfg(feature = "unstable")]
        pub use try_default::TryDefault;

        #[cfg(feature = "layout_checks")]
        pub use crate::trait_group::VerifyLayout;
    }
}

#[cfg(test)]
pub mod tests;
