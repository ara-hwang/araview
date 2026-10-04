use std::env;

fn main() {
    println!("cargo:rerun-if-changed=araview_thumb.def");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{dir}\\araview_thumb.def");
    }
}
