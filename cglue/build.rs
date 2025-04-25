use rustc_version::{version, Version};

fn main() {
    let cfgs = [
        ("1.57.0", "const_panic_on_stable"),
        ("1.65.0", "gats_on_stable"),
        ("1.81.0", "c_unwind_on_stable"),
    ];

    let version = version().unwrap();

    for (v, c) in &cfgs {
        println!("cargo:rustc-check-cfg=cfg({})", c);
        if version >= Version::parse(v).unwrap() {
            println!("cargo:rustc-cfg={}", c);
        }
    }

    let test_cfgs = ["__cglue_force_no_unwind_abi"];

    for c in &test_cfgs {
        println!("cargo:rustc-check-cfg=cfg({})", c);
    }
}
