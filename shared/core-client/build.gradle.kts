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
        jvmMain.dependencies {
            implementation(libs.jna)
        }
        jvmTest.dependencies {
            implementation(kotlin("test"))
            implementation(kotlin("test-junit5"))
            implementation(libs.junit.jupiter)
        }
    }
}

tasks.withType<Test>().configureEach {
    val nativeDll =
        rootProject.layout.projectDirectory
            .dir("build/native/windows-x64")
            .file("nexus_core.dll")
            .asFile
    systemProperty("nexus.native.path", nativeDll.absolutePath)
}
