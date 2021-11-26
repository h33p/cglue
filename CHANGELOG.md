# CGlue changelog

## Changes in 0.2.1:

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

