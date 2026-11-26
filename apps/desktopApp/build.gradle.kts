import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
}

val emberVersion = providers.gradleProperty("emberVersion").getOrElse("0.1.0")

// The Rust core is built outside Gradle by scripts/build-native.ps1. Staging it
// here puts nexus_core.dll into both the app image and the MSI; a build without
// the native step still produces a UI-only artifact.
val nativeCoreDll = rootProject.layout.projectDirectory.file("build/native/windows-x64/nexus_core.dll")
val stageNativeCore =
    tasks.register<Copy>("stageNativeCore") {
        onlyIf { nativeCoreDll.asFile.exists() }
        from(nativeCoreDll)
        into(layout.buildDirectory.dir("app-resources/windows-x64"))
    }
tasks
    .matching {
        it.name == "prepareAppResources" || it.name == "createDistributable" || it.name == "packageMsi"
    }.configureEach {
        dependsOn(stageNativeCore)
    }

kotlin {
    jvmToolchain(21)
}

dependencies {
    implementation(project(":shared:ui"))
    implementation(compose.desktop.currentOs)
}

compose.desktop {
    application {
        mainClass = "io.nexus.desktop.MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Msi)
            packageName = "Ember"
            packageVersion = emberVersion
            // Files under app-resources/<os>-<arch> are copied into the
            // installed application directory and packaged by the MSI.
            appResourcesRootDir.set(layout.buildDirectory.dir("app-resources"))
            description = "Ember desktop application"
            vendor = "Ember"
            copyright = "Copyright 2026 farokh-ra. All rights reserved."
            windows {
                // Per-user installs avoid elevation and keep application data
                // under the user's profile. The upgrade UUID is stable across
                // releases so a newer MSI replaces the previous version.
                perUserInstall = true
                upgradeUuid = "c7c225ee-ae2c-4c58-822d-ea7bda7ac48c"
                menuGroup = "Ember"
                shortcut = true
                menu = true
                dirChooser = true
            }
        }
    }
}
