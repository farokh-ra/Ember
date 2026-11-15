#Requires -Version 5.1
<#
.SYNOPSIS
    Installs the pinned portable Windows toolchain without administrator rights.

.DESCRIPTION
    Installs JDK 21, Gradle 9.9.1, Rust 1.99.1, Android NDK r29, and (only when
    no complete MSVC C++ workload is present) a portable MinGW-w64 toolchain
    under %LOCALAPPDATA%\NEXUS\toolchain.

    The Android SDK itself must already exist at %LOCALAPPDATA%\Android\Sdk with
    accepted licenses and cmdline-tools installed.
#>
[CmdletBinding()]
param(
    [switch]$Force
)

# Native installers write progress and warnings to stderr. Windows PowerShell
# 5.1 treats stderr as terminating when this preference is Stop, so use
# Continue and check every native exit code explicitly.
$ErrorActionPreference = "Continue"
$ToolchainRoot = Join-Path $env:LOCALAPPDATA "NEXUS\toolchain"
$Downloads = Join-Path $ToolchainRoot "downloads"
$JdkHome = Join-Path $ToolchainRoot "jdk\jdk-21.0.13+9"
$GradleHome = Join-Path $ToolchainRoot "gradle\gradle-9.9.1"
$MingwHome = Join-Path $ToolchainRoot "mingw\mingw64"
$CargoHome = Join-Path $ToolchainRoot "cargo"
$RustupHome = Join-Path $ToolchainRoot "rustup"
$AndroidHome = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA "Android\Sdk" }
$AndroidCli = Join-Path $AndroidHome "cmdline-tools\latest\bin\android.exe"
$SdkManager = Join-Path $AndroidHome "cmdline-tools\latest\bin\sdkmanager.bat"
$NdkVersion = "29.0.14206865"
$RustVersion = "1.99.1"

$JdkUrl = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jdk_x64_windows_hotspot_21.0.13_9.zip"
$JdkSha256 = "a4dbaa05fca4bd89d84584e58c459bd732efebb1722af80ab13c6516832004f9"
$GradleUrl = "https://services.gradle.org/distributions/gradle-9.9.1-bin.zip"
$GradleSha256 = "d8a18a7a841dd5df825d35a685fad6f2d7f12ac0c280e57b67c27b4a520ba228"
$MingwUrl = "https://github.com/brechtsanders/winlibs_mingw/releases/download/16.2.0posix-23.1.3-14.0.0-ucrt-r3/winlibs-x86_64-posix-seh-gcc-16.2.0-llvm-23.1.3-mingw-w64ucrt-14.0.0-r3.zip"
$MingwSha256 = "1f04e482797dc5f71fe2875cf2b743bfb9f5fd1de0955fdd3118438c0432ba5f"
$RustupUrl = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
$RustupSha256 = "38027d51d69a5d05493627a0634dbea52d6cd793c8f072d2373d3f4163eec7db"

New-Item -ItemType Directory -Force -Path $ToolchainRoot, $Downloads | Out-Null

function Get-PinnedFile {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$Sha256
    )

    if ((Test-Path -LiteralPath $Destination) -and -not $Force) {
        $existing = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($existing -eq $Sha256) {
            Write-Host "Using cached $(Split-Path $Destination -Leaf)"
            return
        }
        Remove-Item -LiteralPath $Destination -Force
    }

    Write-Host "Downloading $(Split-Path $Destination -Leaf)"
    & curl.exe --silent --show-error -L --fail --retry 3 --retry-delay 2 -o $Destination $Url 2>&1 | ForEach-Object { Write-Output "$_" }
    if ($LASTEXITCODE -ne 0) {
        throw "Download failed: $Url"
    }

    $actual = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256) {
        Remove-Item -LiteralPath $Destination -Force
        throw "Checksum mismatch for $Url. Expected $Sha256, got $actual."
    }
}

function Expand-PinnedZip {
    param(
        [Parameter(Mandatory = $true)][string]$Zip,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    & tar.exe -xf $Zip -C $Destination 2>&1 | ForEach-Object { Write-Output "$_" }
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to extract $Zip"
    }
}

if (-not (Test-Path (Join-Path $JdkHome "bin\java.exe")) -or $Force) {
    $jdkZip = Join-Path $Downloads "OpenJDK21U-jdk_x64_windows_hotspot_21.0.13_9.zip"
    Get-PinnedFile -Url $JdkUrl -Destination $jdkZip -Sha256 $JdkSha256
    Expand-PinnedZip -Zip $jdkZip -Destination (Join-Path $ToolchainRoot "jdk")
} else {
    Write-Host "Using existing JDK 21.0.13+9"
}

if (-not (Test-Path (Join-Path $GradleHome "bin\gradle.bat")) -or $Force) {
    $gradleZip = Join-Path $Downloads "gradle-9.9.1-bin.zip"
    Get-PinnedFile -Url $GradleUrl -Destination $gradleZip -Sha256 $GradleSha256
    Expand-PinnedZip -Zip $gradleZip -Destination (Join-Path $ToolchainRoot "gradle")
} else {
    Write-Host "Using existing Gradle 9.9.1"
}

$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
$msvcPath = ""
if (Test-Path $vswhere) {
    $msvcPath = (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null)
}
$hasMsvc = -not [string]::IsNullOrWhiteSpace($msvcPath)

if ($hasMsvc) {
    Write-Host "MSVC C++ tools detected at $msvcPath"
} else {
    if (-not (Test-Path (Join-Path $MingwHome "bin\gcc.exe")) -or $Force) {
        $mingwZip = Join-Path $Downloads "winlibs-x86_64-posix-seh-gcc-16.2.0-llvm-23.1.3-mingw-w64ucrt-14.0.0-r3.zip"
        Get-PinnedFile -Url $MingwUrl -Destination $mingwZip -Sha256 $MingwSha256
        Expand-PinnedZip -Zip $mingwZip -Destination (Join-Path $ToolchainRoot "mingw")
    } else {
        Write-Host "Using existing MinGW-w64 GCC 16.2.0"
    }
    Write-Host "MinGW is the Windows Rust linker fallback for this non-admin setup."
}

if (-not (Test-Path (Join-Path $CargoHome "bin\rustup.exe")) -or $Force) {
    $rustupInit = Join-Path $Downloads "rustup-init.exe"
    Get-PinnedFile -Url $RustupUrl -Destination $rustupInit -Sha256 $RustupSha256
    $env:RUSTUP_HOME = $RustupHome
    $env:CARGO_HOME = $CargoHome
    & $rustupInit -y --default-toolchain none --profile minimal --no-modify-path 2>&1 | ForEach-Object { Write-Output "$_" }
    if ($LASTEXITCODE -ne 0) {
        throw "rustup-init failed."
    }
} else {
    Write-Host "Using existing rustup"
}

$env:RUSTUP_HOME = $RustupHome
$env:CARGO_HOME = $CargoHome
$env:PATH = "$CargoHome\bin;$env:PATH"
if (-not $hasMsvc) {
    $env:PATH = "$MingwHome\bin;$env:PATH"
    $hostTriple = "x86_64-pc-windows-gnu"
} else {
    $hostTriple = "x86_64-pc-windows-msvc"
}
$toolchain = "$RustVersion-$hostTriple"

& (Join-Path $CargoHome "bin\rustup.exe") toolchain install $toolchain --profile minimal 2>&1 | ForEach-Object { Write-Output "$_" }
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install Rust toolchain $toolchain."
}
& (Join-Path $CargoHome "bin\rustup.exe") default $toolchain 2>&1 | ForEach-Object { Write-Output "$_" }
if ($LASTEXITCODE -ne 0) {
    throw "Failed to select Rust toolchain $toolchain."
}
& (Join-Path $CargoHome "bin\rustup.exe") component add clippy rustfmt --toolchain $toolchain 2>&1 | ForEach-Object { Write-Output "$_" }
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install Rust components for $toolchain."
}
& (Join-Path $CargoHome "bin\rustup.exe") target add aarch64-linux-android x86_64-linux-android --toolchain $toolchain 2>&1 | ForEach-Object { Write-Output "$_" }
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install Android Rust targets for $toolchain."
}

$ndkProperties = Join-Path $AndroidHome "ndk\$NdkVersion\source.properties"
if (-not (Test-Path $ndkProperties) -or $Force) {
    if (Test-Path $AndroidCli) {
        & $AndroidCli sdk install "ndk/$NdkVersion" 2>&1 | ForEach-Object { Write-Output "$_" }
    } elseif (Test-Path $SdkManager) {
        & $SdkManager --sdk_root=$AndroidHome "ndk;$NdkVersion" 2>&1 | ForEach-Object { Write-Output "$_" }
    } else {
        throw "Android cmdline-tools not found under $AndroidHome. Install them first and accept the SDK licenses."
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install Android NDK $NdkVersion."
    }
} else {
    Write-Host "Using existing Android NDK $NdkVersion"
}

$sdkPackages = @(
    @{ Directory = "platforms\android-37.0"; Package = "platforms/android-37.0" },
    @{ Directory = "build-tools\36.0.0"; Package = "build-tools/36.0.0" }
)
foreach ($sdkPackage in $sdkPackages) {
    if (Test-Path (Join-Path $AndroidHome $sdkPackage.Directory)) {
        Write-Host "Using existing Android SDK package $($sdkPackage.Package)"
        continue
    }
    if (Test-Path $AndroidCli) {
        & $AndroidCli sdk install $sdkPackage.Package 2>&1 | ForEach-Object { Write-Output "$_" }
    } elseif (Test-Path $SdkManager) {
        & $SdkManager --sdk_root=$AndroidHome ($sdkPackage.Package.Replace("/", ";")) 2>&1 | ForEach-Object { Write-Output "$_" }
    } else {
        throw "Android cmdline-tools not found under $AndroidHome. Install them first and accept the SDK licenses."
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install Android SDK package $($sdkPackage.Package)."
    }
}

Write-Host ""
Write-Host "Portable toolchain ready under $ToolchainRoot"
Write-Host "Run: . .\scripts\dev-env.ps1"
Write-Host "Then: .\scripts\build.ps1"
