import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

android {
    namespace = "live.akidev.tv"
    compileSdk = 37

    defaultConfig {
        applicationId = "live.akidev.tv"
        minSdk = 26
        targetSdk = 36
        versionCode = 3
        versionName = "1.0.2"
        // Where the app looks for newer releases of itself (the releases `fastlane release` makes); tests can point it elsewhere with -PupdateUrl.
        buildConfigField("String", "UPDATE_URL", "\"${project.findProperty("updateUrl") ?: "https://api.github.com/repos/dishant0406/akidev/releases?per_page=20"}\"")
        // Both CPU types the engine is built for. Many TVs are 32-bit only.
        ndk { abiFilters += listOf("armeabi-v7a", "arm64-v8a") }
    }
    // The upload key lives outside the repository (~/.akidev-keys, the same one as the manga app); without it the release build is only unsigned.
    val keys = Properties().apply { File(System.getProperty("user.home"), ".akidev-keys/keystore.properties").takeIf { it.exists() }?.inputStream()?.use(::load) }
    signingConfigs {
        if (keys.isNotEmpty()) create("upload") {
            storeFile = file(keys.getProperty("storeFile"))
            storePassword = keys.getProperty("storePassword")
            keyAlias = keys.getProperty("keyAlias")
            keyPassword = keys.getProperty("keyPassword")
        }
    }

    buildTypes {
        release {
            signingConfigs.findByName("upload")?.let { signingConfig = it }
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    // Not for this app: it is for TVs (no ChromeOS), it was tested on the release it targets (36; 37 is only the compile level), it keeps nothing in cloud
    // backups (allowBackup is off), it has no launcher icon to shape (the TV uses the banner), and `edit()` is the plain call (no extra library for it).
    lint { disable += listOf("ChromeOsAbiSupport", "OldTargetApi", "DataExtractionRules", "IconLauncherShape", "UseKtx") }
    buildFeatures {
        compose = true
        buildConfig = true
    }
}

dependencies {
    implementation(project(":engine"))
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.ui)
    implementation(libs.media3.okhttp)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.activity.compose)
    implementation(libs.tv.material)
    implementation(libs.coil.compose)
    implementation(libs.coil.network)
    // Installs the baseline profile (src/main/baseline-prof.txt) on the first run of a sideloaded copy; Play does it at install.
    implementation(libs.profileinstaller)

    testImplementation(libs.junit)
    testImplementation(libs.json)
}
