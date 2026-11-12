#Requires -Version 5.1
<#
.SYNOPSIS
    Configures the current PowerShell process to use the pinned NEXUS toolchain.

.DESCRIPTION
    Installs nothing. Run scripts/setup-windows.ps1 first, then dot-source this
    file so the environment changes persist in your shell:

        . .\scripts\dev-env.ps1

    Use -UseMsvc when the machine has a complete Visual Studio C++ toolchain and
    you want rustup to use the x86_64-pc-windows-msvc host instead of MinGW.
#>
[CmdletBinding()]
param(
    [switch]$Quiet,
    [switch]$UseMsvc
)

# Native tools such as java and rustc write version information to stderr.
# Windows PowerShell 5.1 treats that as an error when this preference is Stop,
# so keep Continue and fail explicitly on missing tools.
$ErrorActionPreference = "Continue"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ToolchainRoot = Join-Path $env:LOCALAPPDATA "NEXUS\toolchain"
$JdkHome = Join-Path $ToolchainRoot "jdk\jdk-21.0.13+9"
$GradleHome = Join-Path $ToolchainRoot "gradle\gradle-9.9.1"
$MingwHome = Join-Path $ToolchainRoot "mingw\mingw64"
$CargoHome = Join-Path $ToolchainRoot "cargo"
$RustupHome = Join-Path $ToolchainRoot "rustup"
$AndroidHome = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA "Android\Sdk" }

function Add-ToPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    $parts = $env:PATH -split ";"
    if ($parts -notcontains $Path) {
        $env:PATH = "$Path;$env:PATH"
    }
}

if (-not (Test-Path (Join-Path $JdkHome "bin\java.exe"))) {
    throw "JDK 21 not found at $JdkHome. Run .\scripts\setup-windows.ps1."
}
if (-not (Test-Path (Join-Path $GradleHome "bin\gradle.bat"))) {
    throw "Gradle 9.9.1 not found at $GradleHome. Run .\scripts\setup-windows.ps1."
}
if (-not (Test-Path (Join-Path $CargoHome "bin\cargo.exe"))) {
    throw "Rust not found at $CargoHome. Run .\scripts\setup-windows.ps1."
}

$env:JAVA_HOME = $JdkHome
$env:GRADLE_HOME = $GradleHome
$env:ANDROID_HOME = $AndroidHome
$env:ANDROID_SDK_ROOT = $AndroidHome
$env:ANDROID_NDK_HOME = Join-Path $AndroidHome "ndk\29.0.14206865"
$env:CARGO_HOME = $CargoHome
$env:RUSTUP_HOME = $RustupHome

Add-ToPath (Join-Path $JdkHome "bin")
Add-ToPath (Join-Path $GradleHome "bin")
Add-ToPath (Join-Path $CargoHome "bin")

$hasMingw = Test-Path (Join-Path $MingwHome "bin\gcc.exe")
if (-not $UseMsvc -and $hasMingw) {
    Add-ToPath (Join-Path $MingwHome "bin")
    $env:RUSTUP_TOOLCHAIN = "1.99.1-x86_64-pc-windows-gnu"
} else {
    Remove-Item Env:\RUSTUP_TOOLCHAIN -ErrorAction SilentlyContinue
}

if (Test-Path (Join-Path $AndroidHome "platforms\android-37.0")) {
    $localProperties = Join-Path $RepoRoot "local.properties"
    $sdkLine = "sdk.dir=$($AndroidHome.Replace('\', '\\'))"
    $current = if (Test-Path $localProperties) { Get-Content -LiteralPath $localProperties -Raw } else { "" }
    if ($null -eq $current -or $current.Trim() -ne $sdkLine) {
        $utf8 = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($localProperties, "$sdkLine`n", $utf8)
    }
}

if (-not $Quiet) {
    $javaVersion = (& (Join-Path $JdkHome "bin\java.exe") -version 2>&1 | Select-Object -First 1)
    Write-Host "NEXUS developer environment"
    Write-Host "  JDK:     $javaVersion"
    Write-Host "  Gradle:  $GradleHome"
    Write-Host "  Rust:    $(& (Join-Path $CargoHome "bin\rustc.exe") --version)"
    Write-Host "  Cargo:   $(& (Join-Path $CargoHome "bin\cargo.exe") --version)"
    Write-Host "  Android: $AndroidHome"
    Write-Host "  NDK:     $env:ANDROID_NDK_HOME"
    if ($env:RUSTUP_TOOLCHAIN) {
        Write-Host "  Rust host override: $env:RUSTUP_TOOLCHAIN (MinGW fallback)"
    }
    Write-Host ""
    Write-Host "For this shell only. To persist, run: . .\scripts\dev-env.ps1"
}
