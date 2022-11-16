use rustc_version::{version, Version};

fn main() {
    if version().unwrap() >= Version::parse("1.65.0").unwrap() {
        println!("cargo:rustc-cfg=gats_on_stable");
    }
}
