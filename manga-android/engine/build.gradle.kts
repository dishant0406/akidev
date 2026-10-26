// The Rust engine: the generated Kotlin bindings and the native library, both made by scripts/build-engine.sh.
plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "live.akidev.manga.engine"
    compileSdk = 37
    defaultConfig { minSdk = 29 }
}

dependencies {
    api(variantOf(libs.jna) { artifactType("aar") })
    api(libs.coroutines)
}
