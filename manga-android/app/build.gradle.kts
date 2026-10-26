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
    // Phones with 32-bit ARM or Intel processors are not supported (the engine is arm64 only). The app keeps nothing in cloud backups (allowBackup is off), so it needs no extraction rules. Target 36 is the release the app was tested on; 37 is only the compile level Haze needs.
    lint { disable += listOf("ChromeOsAbiSupport", "OldTargetApi", "DataExtractionRules") }
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
