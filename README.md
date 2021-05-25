# CGlue

If all code is glued together, our glue is the safest in the market.

## FFI-safe trait generation, helper structures, and more!

CGlue offers an easy way to ABI (application binary interface) safety. Just a few annotations and your trait is ready to go!

```rust
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

    let obj = cglue_obj!(info as InfoPrinter);

    use_info_printer(&obj);
}
```

A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.

This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `cglue_obj` macro:

```rust
let obj = CGlueTraitObjInfoPrinter::from(&mut info).into_opaque();
```

`cglue_trait` annotation generates a `CGlueVtblInfoPrinter` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.

But that's not all, you can also group traits together!

```rust

```
