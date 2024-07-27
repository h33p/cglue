# CGlue changelog

## Changes in 0.3.0:

### [Stabilize `task` feature](https://github.com/h33p/cglue/blob/e6458ae5542daa489561495fb6c613307bb80001/cglue/src/task/mod.rs)

Enable with `task` feature. Provides C equivalents for `Waker`, `RawWaker`, `RawWakerVtable`.

### [Trait alias within trait groups](https://github.com/h33p/cglue/commit/35b0da5bdf6cfe6ecaeedf07ba795d618113477f)

Enables specifying different generic instantiations of the same trait in the same group. Example:

```rust
cglue_trait_group!(TestGroupGen, TT<u8>, { TT<usize> = TTUsize, TT<u64> = TTUSixtyFour });
cglue_impl_group!(SA, TestGroupGen, { TT<usize> = TTUsize });
```

### [Support traits with associated types](https://github.com/h33p/cglue/commit/5d26c373bd49e935dc65b4434bb6593e2109b8fc)

The following now compiles:

```rust
#[cglue_trait]
pub trait WithAssoc<T> {
	type AssocTy: Clone;

	fn with_assoc(&self, assoc: &Self::AssocTy) -> T;
}
```

### [Add Future support + Pin Self parameters](https://github.com/h33p/cglue/commit/6c1662a6db80390690361804626d31b72834ea3c)

The following now works:

```rust
async fn hii() -> u64 {
    42
}

let obj = trait_obj!(hii() as Future);

assert_eq!(pollster::block_on(obj), 42);
```

### [Add futures Stream+Sink support](https://github.com/h33p/cglue/commit/609bc203fb4a96541a64be1a8098b9e93a631e4e)

The following now works:

```rust
let items = [42, 43, 42];

let obj = trait_obj!(futures::stream::iter(items) as Stream);

assert_eq!(pollster::block_on(obj.collect::<Vec<_>>()), items);
```

### [Fix #17](https://github.com/h33p/cglue/commit/2d917012db0caa738eb70a89cb1c16f3ec622fb6)

## Changes in 0.2.14:

[Add unstable task feature](https://github.com/h33p/cglue/commit/9fbee903963b1b407ca218609e43a65cfd1eb219)

[Automatically wrap 'Into<T>' arguments](https://github.com/h33p/cglue/commit/081c590f4eb97b1be10eaeaa9cbf87e7278ea8de)

## Changes in 0.2.12:

[Initial support for GAT lifetimes](https://github.com/h33p/cglue/commit/1a8098181896bb730d276aea59464d577e5d8927)

## Changes in 0.2.11:

[Fix a safety bug with slices](https://github.com/h33p/cglue/commit/cbb25367cf7867f07b0a2c7718f894de437f22eb)

## Changes in 0.2.10:

[Rename Tup to CTup](https://github.com/h33p/cglue/commit/0c4eeabc9196a7796216a57d40b684f2e68f4d58)

## Changes in 0.2.9 (yanked):

[Add C tuples](https://github.com/h33p/cglue/commit/41a4c7d2ad3991bd17d2572a52809586f57724a1)

## Changes in 0.2.8:

[Allow to provide custom implemenetations for generics functions](https://github.com/h33p/cglue/commit/d9b5a765f4c5a8c63f4a4755bb50e85b4d2ca928)

Re-export `custom_impl` macro.

## Changes in 0.2.7:

[Add more helpers to CVec](https://github.com/h33p/cglue/commit/5658698ba2c343f746d4cba14a774f02ed642e78)

[Parse different expressions in the cast macros](https://github.com/h33p/cglue/commit/ef9f650d6c4e309f51ae996c9672473942809621)

[Expose CArcSome](https://github.com/h33p/cglue/commit/849ca7c11901c732541e4d0641cb35501ed125d6):

- This is equivalent to `Arc`, and is essentially a pre-null-checked version of `CArc`.

## Changes in 0.2.5:

[fix no\_std build](https://github.com/h33p/cglue/commit/6f99749708f3a38825ec107b9a64c38870010d40).

## Changes in 0.2.4:

[Make cglue generated exports easier to import](https://github.com/h33p/cglue/commit/788fbce9c584e699a56bd5a16d405d52e2119714):

- `cglue_##trait_or_groupname` module is exposed as public that contains all types that are being re-exported to parent module.

- In the future, these types may not be re-exported anymore, and code generator may rely on `cglue_##trait_or_groupname` to exist in scope for cleaner code generation.

[Add boxed slice, CVec, and add more serde support](https://github.com/h33p/cglue/commit/fd549808f6f3bb0477bd394831d4e8dd599c757c).

[Compatible with official abi\_stable](https://github.com/h33p/cglue/commit/7d9147df560412a49ab767928a2c6fcbc72bff2b):

- Users should now use `cglue::trait_group::c_void` as the `c_void` type.

- Technically breaks semver, but it is not possible to do anything with `c_void` anyways.

## Changes in 0.2.3:

[Make formatting traits FFI-safe](https://github.com/h33p/cglue/commit/dd86a2145bceb48075f560f69c10686e71634756):

- All standard fmt traits are exposed.

- Only Debug and Display are in the prefix.

- Not full formatting functionality is preserved across FFI boundary.

[Add extra customization to C function impls](https://github.com/h33p/cglue/blob/dd86a2145bceb48075f560f69c10686e71634756/cglue-gen/src/ext/core/fmt.rs#L21).

### Changes cglue-bindgen 0.2.2:

[Make C++ generator to be C++11 friendly](https://github.com/h33p/cglue/commit/6457ad9e0ffb945822f76d8ad08c60743841dd7b).

## Changes in 0.2.2:

Fix no\_std compilation.

## Changes in 0.2.0:

[Rework code generation](https://github.com/h33p/cglue/commit/e589a0115ec343c4538804d3d8ef76ca101a112a):

- Make code generator emit C functions that take in a single object that contains both the object and the return context. This simplifies usage from C/C++.

- Remove zero wrapper optimization. The above is incompatible with it.

- Remove CtxBox, as context has been moved into the container object.

[Context is now always `Clone + Send + Sync`](https://github.com/h33p/cglue/commit/bf24eaec1d518ff82356a05646ecb3af4f4b177b).

Ergonomic C/C++ wrappers:

- In C++ trait objects and groups have member functions and destructors defined. Trait objects and groups themselves are ABI-incompatible when passed by value.

- In C there are inline functions for invoking behavior. Some of the functions accept void pointers, because they are compatible with multiple variations of the same CGlue object.

- Ability to specify default container and context types, so that the wrappers become more compact.

- Manual cleanup is no longer supported as bindgen has become more complex.

[Somewhat tested with miri](https://github.com/h33p/cglue/blob/af6ab0dd2b2ecfa24e8f67ba9246c0079f654f6e/.github/workflows/build.yml#L121):

- Stacked borrows are disabled.

- ABI checks have been patched out, because otherwise rust evaluator does not accept type erasure.

[Vtable-only functions](https://github.com/h33p/cglue/commit/f9f600fb3accb7d7f1970507c79786eade12e78a):

- Provide Rust functionality in C/C++ with slightly different types.

[Wrap strings and slices in return types](https://github.com/h33p/cglue/commit/c8a607e68a851321a4bc288491e879e34d541bd2).

[Unstable feature to auto-impl groups](https://github.com/h33p/cglue/commit/af6ab0dd2b2ecfa24e8f67ba9246c0079f654f6e):

- Auto-enables the widest set of optional traits for any given type.

- Makes `cglue_impl_group!` a no-op.

[Runtime ABI/API validation with abi\_stable](https://github.com/h33p/cglue/commit/5b75b31a3dfb35967721d94df2e83f3ced8be9c2).

[Remove no\_empty\_retwrap feature](https://github.com/h33p/cglue/commit/4e2703df12c1f69b1aa4e02f8328d660ef0bf17b).

[Replace CArc with COptArc, make old CArc private](https://github.com/h33p/cglue/commit/b5caf2229fe236e2697d3b5b15b58a92b59bd6d4):

- COptArc was always the prefered choice, as it allowed to also represent `None` in the same amount of space.

