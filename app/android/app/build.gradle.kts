plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "dev.gravitaltalk.gravital_talk_app"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "dev.gravitaltalk.gravital_talk_app"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

// ── Librería nativa Rust (libgravital_talk_ffi.so) ────────────────────────
// Compila los .so con cargo-ndk antes del build de Android cuando:
//   - existe un NDK instalado, y
//   - el script `scripts/flutter-android-libs.sh` está presente.
// Si no hay NDK, el build continúa (la app usa el motor demo).
tasks.register<Exec>("buildRustLibs") {
    description = "Compila libgravital_talk_ffi.so (cargo-ndk) para los ABIs Android"
    val ndkDirs = file("${System.getenv("ANDROID_HOME") ?: ""}/ndk")
    val genLibs = file("src/main/jniLibs")
    outputs.dir(genLibs)
    val cmd = mutableListOf(
        "bash",
        file("../../scripts/flutter-android-libs.sh").absolutePath,
    )
    commandLine(cmd)
    enabled = ndkDirs.isDirectory || !System.getenv("ANDROID_NDK_HOME").isNullOrEmpty()
    doFirst {
        if (!enabled) {
            logger.warn("NDK no detectado: sin .so nativos (motor demo). Usa scripts/flutter-android-libs.sh o el workflow flutter-android.yml.")
        }
    }
}

tasks.configureEach {
    if (name == "preBuild") {
        dependsOn("buildRustLibs")
    }
}
