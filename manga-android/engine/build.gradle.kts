// The Rust engine: the generated Kotlin bindings and the native library, both made by scripts/build-engine.sh.
plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "live.akidev.manga.engine"
    compileSdk = 37
    defaultConfig { minSdk = 29 }
    // The generated bindings choose their cleaner at run time (the one that needs Android 13 only where it exists): lint cannot see that.
    lint { disable += "NewApi" }
}

dependencies {
    api(variantOf(libs.jna) { artifactType("aar") })
    api(libs.coroutines)
}
