import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("io.github.takahirom.roborazzi") // screenshots of the UI rendered on the JVM (no emulator needed)
}

// Release signing comes from android/keystore.properties (not in git): storeFile, storePassword, keyAlias, keyPassword.
val signing = Properties().apply {
    rootProject.file("keystore.properties").takeIf { it.exists() }?.inputStream()?.use { load(it) }
}

android {
    namespace = "com.faizzyhon.gale"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.faizzyhon.gale"
        minSdk = 29 // Android 10: MediaStore downloads without storage permission
        targetSdk = 36
        versionCode = 2
        versionName = "0.6.0"
    }

    // full: the APK on GitHub. play: Google Play build (no YouTube downloads, yt-dlp only updates with the app).
    flavorDimensions += "store"
    productFlavors {
        create("full") { dimension = "store"; buildConfigField("boolean", "PLAY", "false") }
        create("play") { dimension = "store"; buildConfigField("boolean", "PLAY", "true") }
    }

    signingConfigs {
        if (signing.isNotEmpty()) create("release") {
            storeFile = rootProject.file(signing.getProperty("storeFile"))
            storePassword = signing.getProperty("storePassword")
            keyAlias = signing.getProperty("keyAlias")
            keyPassword = signing.getProperty("keyPassword")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false // yt-dlp/libtorrent bindings use reflection and JNI names
            signingConfig = signingConfigs.findByName("release")
        }
    }

    // One APK per processor type: the bundled Python, FFmpeg and libtorrent are large per ABI.
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            isUniversalApk = false
        }
    }

    packaging {
        jniLibs.useLegacyPackaging = true // youtubedl-android runs its bundled binaries from the lib folder
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true; buildConfig = true }
    testOptions {
        unitTests.isReturnDefaultValues = true
        unitTests.isIncludeAndroidResources = true // Robolectric screenshots
        // Optional Maven mirror for Robolectric's Android jars (it downloads them itself, outside Gradle).
        unitTests.all { t ->
            System.getenv("ROBOLECTRIC_REPO")?.let { t.systemProperty("robolectric.dependency.repo.url", it) }
            t.forkEvery = 1 // libtorrent's native library and Robolectric's native graphics crash when sharing a JVM
        }
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.12.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.12.4")
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.10.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.10.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    implementation("com.squareup.okhttp3:okhttp:5.3.2")

    implementation("io.github.junkfood02.youtubedl-android:library:0.18.1")
    implementation("io.github.junkfood02.youtubedl-android:ffmpeg:0.18.1")

    val lt = "2.1.0-39"
    implementation("org.libtorrent4j:libtorrent4j:$lt")
    implementation("org.libtorrent4j:libtorrent4j-android-arm64:$lt")
    implementation("org.libtorrent4j:libtorrent4j-android-arm:$lt")
    implementation("org.libtorrent4j:libtorrent4j-android-x86_64:$lt")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.16.1")
    testImplementation("io.github.takahirom.roborazzi:roborazzi:1.77.0")
    testImplementation("io.github.takahirom.roborazzi:roborazzi-compose:1.77.0")
    testImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
    testImplementation("org.json:json:20250517")
    testImplementation("org.libtorrent4j:libtorrent4j-linux:$lt") // native library for the JVM torrent test
    testImplementation("com.squareup.okhttp3:mockwebserver3:5.3.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
}
