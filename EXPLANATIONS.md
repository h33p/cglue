# CGlue codegen explanations

Some of the more important details behind certain decisions of CGlue code generation.

## Function bounds

### Lifetimes in cfunc definition vs. cfunc vtable declaration

The key link between the two is that the cfunc must be valid for all possible lifetimes. Thus, in declaration we have the following syntax:


```rust
do_thing: for<'a> extern "C" fn(&'a mut T)
```

While the definition uses the following:


```rust
extern "C" fn cglue_do_thing<T: Trait>(&mut T) {
    // ...
}
```

It looks different, but `cglue_do_thing` is secretly valid for all lifetimes of T (`for<'a>`).

This becomes very important when we have additional type bounds required for the function, such as the following:

```rust
extern "C" fn cglue_do_thing<
        'a,
	    T: Trait + Into<CBoxRef<'a, T::SubObj>>
	>(&'a mut T) -> CBoxRef<'a, T::SubObj> {
	// ...
}
```

The type bound becomes misleading, because it binds `T` to that single lifetime, as opposed to all possible lifetimes (and type bound thus becomes unique on each individual lifetime). The function's type becomes incompatible with default implementation for the vtable creation. To fix this, we must bind the type to any possible lifetime, as opposed to the particular function's lifetime:

```
extern "C" fn cglue_do_thing<'a, T: Trait>(&'a mut T) -> CBoxRef<'a, T::SubObj>
	where for<'b> T: Into<CBoxRef<'b, T::SubObj>>
{
	// ...
}
```

This will apply the exact same bound for all lifetimes `'a`, which makes the function types uniform and compatible to be with vtable creation.

This is very important, because all instances of a function must resolve to the same underlying function, otherwise we'd have non-deterministic number of cfunc instantiations with slightly different characteristics. This becomes extremely important in GATs.

### Lifetime changes when wrapping

Within cfuncs lifetimes in associated type definitions (`type Type<'a>: 'a`) take precedence over lifetimes defined by trait functions. This is to make types the source of truth.

During codegen, we may encounter a function as follows:

```rust
fn do_something<'b>(&'b mut self) -> Self::Type<'b> {
    // ...
}
```

Since `Self::Type` was defined with lifetime `'a`, the implementation will match `'a` to `'b` and replace `'b` with `'a` within cfuncs.
