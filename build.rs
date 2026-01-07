fn main() {
    // Only for Android targets
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // Camera2 NDK
        println!("cargo:rustc-link-lib=camera2ndk");
        // Media NDK (AImageReader / AImage)
        println!("cargo:rustc-link-lib=mediandk");

        // Обычно не нужно, но пусть будет явным:
        println!("cargo:rustc-link-lib=android");
        println!("cargo:rustc-link-lib=log");
    }
}
