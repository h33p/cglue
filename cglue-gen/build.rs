fn main() {
    println!("cargo:rustc-check-cfg=cfg(__cglue_force_no_unwind_abi)");
}
