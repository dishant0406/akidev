// The Rust engine: the generated Kotlin bindings and the native library, both made by scripts/build-engine.sh.
plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "live.akidev.tv.engine"
    compileSdk = 37
    defaultConfig { minSdk = 26 }
    // The generated bindings choose their cleaner at run time: lint cannot see that.
    lint { disable += "NewApi" }
}

dependencies {
    api(variantOf(libs.jna) { artifactType("aar") })
    api(libs.coroutines)
}
