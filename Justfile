build-android:
  cargo ndk build --target aarch64-linux-android --platform 29 --no-default-features --features=cli --bin asimov-camera-cataloger

run-android:
  cargo ndk run --target aarch64-linux-android --platform 29 --no-default-features --features=cli --bin asimov-camera-cataloger
