plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

android {
    namespace = "live.akidev.manga"
    compileSdk = 37

    defaultConfig {
        applicationId = "live.akidev.manga"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
        // The engine is built for 64-bit ARM phones only (and the emulator on an Apple-silicon Mac); JNA's own library comes for each of these.
        ndk { abiFilters += "arm64-v8a" }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    buildFeatures { compose = true }
}

dependencies {
    implementation(project(":design"))
    implementation(project(":engine"))
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.viewmodel)
    implementation(libs.lifecycle.runtime)
    implementation(libs.work.runtime)
}
