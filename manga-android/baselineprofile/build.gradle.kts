// Records the baseline profile (the code a start and the main screens run) on a connected phone: `./gradlew :app:generateReleaseBaselineProfile`.
plugins {
    alias(libs.plugins.android.test)
    alias(libs.plugins.baselineprofile)
}

android {
    namespace = "live.akidev.manga.baselineprofile"
    compileSdk = 37
    defaultConfig {
        minSdk = 29
        targetSdk = 36
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    targetProjectPath = ":app"
}

baselineProfile {
    useConnectedDevices = true
}

dependencies {
    implementation(libs.test.junit)
    implementation(libs.uiautomator)
    implementation(libs.benchmark.macro)
}
