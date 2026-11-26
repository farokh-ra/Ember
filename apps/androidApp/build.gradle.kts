import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

val emberVersion = providers.gradleProperty("emberVersion").getOrElse("0.1.0")

// Android versionCode is derived from the semantic version unless it is
// overridden explicitly: 1.2.3 -> 10203. The override exists for releases that
// need a distinct code without changing the public version name.
val derivedVersionCode =
    emberVersion
        .split(".")
        .let { parts ->
            (parts.getOrNull(0)?.toIntOrNull() ?: 0) * 10000 +
                (parts.getOrNull(1)?.toIntOrNull() ?: 0) * 100 +
                (parts.getOrNull(2)?.toIntOrNull() ?: 0)
        }
val emberVersionCode = providers.gradleProperty("emberVersionCode").getOrElse("$derivedVersionCode").toInt()

// Release signing is supplied by the environment or an untracked
// signing.properties file. Without it, assembleRelease still produces an
// unsigned artifact for local verification, but release publishing requires
// all four values.
val signingPropertiesFile = rootProject.file("signing.properties")
val signingProperties =
    Properties().apply {
        if (signingPropertiesFile.isFile) {
            signingPropertiesFile.inputStream().use { load(it) }
        }
    }

fun releaseSecret(
    property: String,
    environment: String,
): String? =
    providers.gradleProperty(property).orNull?.takeIf { it.isNotBlank() }
        ?: System.getenv(environment)?.takeIf { it.isNotBlank() }
        ?: signingProperties.getProperty(property)?.takeIf { it.isNotBlank() }

val releaseKeystore = releaseSecret("emberKeystore", "EMBER_ANDROID_KEYSTORE")
val releaseStorePassword = releaseSecret("emberStorePassword", "EMBER_ANDROID_STORE_PASSWORD")
val releaseKeyAlias = releaseSecret("emberKeyAlias", "EMBER_ANDROID_KEY_ALIAS")
val releaseKeyPassword = releaseSecret("emberKeyPassword", "EMBER_ANDROID_KEY_PASSWORD")
val releaseSigningReady =
    listOf(releaseKeystore, releaseStorePassword, releaseKeyAlias, releaseKeyPassword)
        .all { !it.isNullOrBlank() }

android {
    namespace = "io.nexus.android"
    compileSdk = 37

    defaultConfig {
        applicationId = "io.nexus.android"
        minSdk = 28
        targetSdk = 37
        versionCode = emberVersionCode
        versionName = emberVersion
    }

    signingConfigs {
        if (releaseSigningReady) {
            create("release") {
                storeFile = rootProject.file(releaseKeystore!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }

    sourceSets {
        getByName("main") {
            // scripts/build-native.ps1 creates the ABI-named JNI library layout.
            // A build without the native step still produces a UI-only artifact.
            jniLibs.srcDir(rootProject.file("build/native/android-jni"))
        }
    }
}

dependencies {
    implementation(project(":shared:ui"))
    implementation(libs.compose.foundation)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
}
