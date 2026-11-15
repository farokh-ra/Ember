import io.gitlab.arturbosch.detekt.Detekt

plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.kmp.library) apply false
    alias(libs.plugins.compose.compiler) apply false
    alias(libs.plugins.compose.multiplatform) apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.detekt)
    alias(libs.plugins.spotless)
}

// AGP 9 compiles Android Kotlin with its built-in Kotlin support. This
// buildscript dependency raises the Kotlin Gradle plugin used by AGP to the
// project pin in gradle/libs.versions.toml.
buildscript {
    dependencies {
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21")
    }
}

allprojects {
    group = "io.nexus"
    version = "0.1.0"
}

subprojects {
    tasks.withType<Test>().configureEach {
        useJUnitPlatform()
    }
}

spotless {
    kotlin {
        target("**/*.kt")
        targetExclude("**/build/**", "**/.gradle/**")
        ktlint(libs.versions.ktlint.get())
        trimTrailingWhitespace()
        endWithNewline()
    }
    kotlinGradle {
        target("**/*.kts")
        targetExclude("**/build/**", "**/.gradle/**")
        ktlint(libs.versions.ktlint.get())
        trimTrailingWhitespace()
        endWithNewline()
    }
    format("misc") {
        target(
            "*.yml",
            "*.yaml",
            "*.toml",
            "*.properties",
            "*.xml",
            ".editorconfig",
            ".gitattributes",
            ".gitignore",
            ".gitmessage",
        )
        targetExclude("**/build/**", "**/.gradle/**")
        trimTrailingWhitespace()
        endWithNewline()
    }
}

detekt {
    buildUponDefaultConfig = true
    parallel = true
    config.setFrom(files("$rootDir/config/detekt/detekt.yml"))
    source.setFrom(
        fileTree(rootDir) {
            include("shared/**/src/**/*.kt", "apps/**/src/**/*.kt")
            exclude("**/build/**")
        },
    )
}

tasks.withType<Detekt>().configureEach {
    jvmTarget = "21"
    reports {
        html.required.set(true)
        sarif.required.set(true)
    }
}
