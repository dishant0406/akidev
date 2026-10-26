// The Aki design language: tokens, glass and the small components every screen is built from.
plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.compose.compiler)
}

android {
    namespace = "live.akidev.manga.design"
    compileSdk = 37
    defaultConfig { minSdk = 29 }
    buildFeatures { compose = true }
}

dependencies {
    api(platform(libs.compose.bom))
    api(libs.compose.ui)
    api(libs.compose.foundation)
    api(libs.haze)
    api(libs.haze.glass)
}
