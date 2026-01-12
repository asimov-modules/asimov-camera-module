fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-lib=camera2ndk");
        println!("cargo:rustc-link-lib=mediandk");

        println!("cargo:rustc-link-lib=android");
        println!("cargo:rustc-link-lib=log");
    }
}
