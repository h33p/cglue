use rustc_version::{version, Version};

fn main() {
    let version = version().unwrap();
    if version >= Version::parse("1.57.0").unwrap() {
        println!("cargo:rustc-cfg=const_panic_on_stable");
    }
    if version >= Version::parse("1.65.0").unwrap() {
        println!("cargo:rustc-cfg=gats_on_stable");
    }
}
