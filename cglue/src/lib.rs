//!
//! # CGlue
//!
//! If all code is glued together, our glue is the safest in the market.
//!
//! ## FFI-safe trait generation, helper structures, and more!
//!
//! CGlue offers an easy way to ABI (application binary interface) safety. Just a few annotations and your trait is ready to go!
//!
//! ```rust
//! use cglue_macro::*;
//!
//! #[cglue_trait]
//! pub trait InfoPrinter {
//!     fn print_info(&self);
//! }
//!
//! struct Info {
//!     value: usize
//! }
//!
//! impl InfoPrinter for Info {
//!     fn print_info(&self) {
//!         println!("Info struct: {}", self.value);
//!     }
//! }
//!
//! fn use_info_printer(printer: &impl InfoPrinter) {
//!     println!("Printing info:");
//!     printer.print_info();
//! }
//!
//! fn main() {
//!     let mut info = Info {
//!         value: 5
//!     };
//!
//!     let obj = trait_obj!(&mut info as InfoPrinter);
//!
//!     use_info_printer(&obj);
//! }
//! ```
//!
//! A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.
//!
//! This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `trait_obj` macro:
//!
//! ```ignore
//! let obj = CGlueTraitObjInfoPrinter::from(&mut info).into_opaque();
//! ```
//!
//! `cglue_trait` annotation generates a `CGlueVtblInfoPrinter` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.
//!
//! But that's not all, you can also group traits together!
//!
//! ```
//! use cglue_macro::*;
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
//! // Define a trait group with `InfoPrinter` as mandatory trait, and
//! // `InfoChanger` with `InfoDeleter` as optional traits.
//! cglue_trait_group!(InfoGroup, InfoPrinter, { InfoChanger, InfoDeleter });
//!
//! // Implement the group for `Info` structure, defining
//! // only that `InfoChanger` is optionally implemented.
//! cglue_impl_group!(Info, InfoGroup, InfoChanger);
//!
//! let mut info = Info { value: 5 };
//!
//! let mut obj = group_obj!(info as InfoGroup);
//!
//! // Object does not implement `InfoDeleter`
//! assert!(as_ref!(&obj impl InfoDeleter).is_none());
//!
//! change_info(&mut cast!(obj impl InfoChanger).unwrap(), 20);
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
//! As for details, commonly used Rust structures are automatically wrapped in a way that works.
//!
//! For instance, slices get split up into pointer and size pairs:
//!
//! ```ignore
//! fn with_slice(&self, slice: &[usize]) {}
//!
//! // Generated vtable entry:
//!
//! with_slice: extern "C" fn(&T, slice: *const usize, slice_size: usize),
//! ```
//!
//! `Option` types that can not have [nullable pointer optimization](https://doc.rust-lang.org/nomicon/ffi.html#the-nullable-pointer-optimization) are wrapped into [COption](crate::option::COption):
//!
//! ```ignore
//! fn non_npo_option(&self, opt: Option<usize>) {}
//!
//! // Generated vtable entry:
//!
//! non_npo_option: extern "C" fn(&T, opt: Option<usize>),
//! ```
//!
//! `Result` is automatically wrapped into [CResult](crate::result::CResult):
//!
//! ```ignore
//! fn with_cresult(&self) -> Result<usize, ()> {}
//!
//! // Generated vtable entry:
//!
//! with_cresult: extern "C" fn(&T) -> CResult<usize, ()>,
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
//! with_int_result: extern "C" fn(&T, ok_out: &mut MaybeUninit<usize>) -> i32,
//! ```
//!
//! All wrapping and conversion is handled transparently behind the scenes, with user's control.

pub mod arc;
pub mod boxed;
pub mod callback;
pub mod option;
pub mod repr_cstring;
pub mod result;
pub mod trait_group;

#[cfg(test)]
pub mod tests;
