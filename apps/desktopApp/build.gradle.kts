import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
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
            packageName = "NEXUS"
            packageVersion = "0.1.0"
            description = "NEXUS desktop application"
            vendor = "NEXUS"
        }
    }
}
