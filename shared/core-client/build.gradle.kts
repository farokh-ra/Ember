import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.kmp.library)
}

kotlin {
    jvm()
    android {
        namespace = "io.nexus.core.client"
        compileSdk = 37
        minSdk = 28
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_21)
        }
    }
    jvmToolchain(21)

    sourceSets {
        commonMain.dependencies {
            api(project(":shared:core-api"))
        }
        jvmTest.dependencies {
            implementation(kotlin("test"))
            implementation(kotlin("test-junit5"))
            implementation(libs.junit.jupiter)
        }
    }
}
