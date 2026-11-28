#Requires -Version 5.1
<#
.SYNOPSIS
    Regenerates Gradle dependency locks and SHA-256 verification metadata.

.DESCRIPTION
    Run this only after an intentional dependency change. It resolves the same
    task set used by CI and the release pipeline, writes one gradle.lockfile per
    project plus gradle/verification-metadata.xml, and leaves the files staged
    for review. Commit both the lockfiles and the metadata in the same change.

    The command intentionally includes debug and release variants and the shared
    JVM tests so that a later CI resolution does not discover a module that is
    absent from the lock or verification state.
#>
[CmdletBinding()]
param(
    [switch]$NoDaemon
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "dev-env.ps1") -Quiet

$arguments = @(
    "--console=plain",
    "--write-locks",
    "--write-verification-metadata", "sha256",
    "spotlessCheck",
    "detekt",
    ":apps:androidApp:assembleDebug",
    ":apps:androidApp:assembleRelease",
    ":apps:androidApp:bundleRelease",
    ":apps:desktopApp:createDistributable",
    ":shared:model:jvmTest",
    ":shared:core-api:jvmTest",
    ":shared:core-client:jvmTest",
    ":shared:presentation:jvmTest",
    ":shared:ui:jvmTest"
)
if ($NoDaemon) {
    $arguments = @("--no-daemon") + $arguments
}

Push-Location $RepoRoot
try {
    & .\gradlew.bat @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Gradle lock generation failed with exit code $LASTEXITCODE."
    }

    Write-Host ""
    Write-Host "Updated dependency state:"
    Get-ChildItem -Recurse -Filter gradle.lockfile |
        ForEach-Object { Write-Host "  $($_.FullName.Substring($RepoRoot.Length + 1))" }
    Get-ChildItem gradle\verification-metadata.xml -ErrorAction SilentlyContinue |
        ForEach-Object { Write-Host "  gradle\verification-metadata.xml" }
    Write-Host ""
    Write-Host "Review the diff, then run .\scripts\build.ps1 -WithDependencyScan."
} finally {
    Pop-Location
}
