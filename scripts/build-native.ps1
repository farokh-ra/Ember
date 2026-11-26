#Requires -Version 5.1
<#
.SYNOPSIS
    Builds the native Ember core library and C ABI harness.

.DESCRIPTION
    Builds `nexus-ffi` for Windows and/or Android, copies each artifact under
    the stable product name `nexus_core`, and compiles and runs the C harness
    against the Windows library.

    Rust's cdylib artifact is `nexus_ffi`; the application, JNI, and JNA load
    `nexus_core`, so this script performs the rename as part of packaging.
#>
[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [ValidateSet("Windows", "Android", "All")]
    [string]$Target = "All",
    [switch]$SkipHarness,
    [switch]$Locked
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$NativeRoot = Join-Path $RepoRoot "build\native"
$ProfileDir = if ($Configuration -eq "Debug") { "debug" } else { "release" }

. (Join-Path $PSScriptRoot "dev-env.ps1") -Quiet

<#
Windows PowerShell 5.1 turns native stderr into a terminating error when
$ErrorActionPreference is Stop. Run native tools with Continue and check the
exit code explicitly.
#>
function Invoke-NativeStep {
    param(
        [Parameter(Mandatory = $true)][string]$Step,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Action 2>&1 | ForEach-Object { Write-Output "$_" }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }

    if ($exitCode -ne 0) {
        throw "$Step failed with exit code $exitCode."
    }
}

function Copy-NativeArtifact {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Native artifact not found: $Source"
    }
    $directory = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
    Write-Host "  $Destination"
}

function Get-NativeSymbols {
    param([Parameter(Mandatory = $true)][string]$Path)

    # Prefer the NDK's llvm-nm: it reads both PE and ELF and is present on every
    # release runner even when MinGW is not installed. Fall back to PATH tools.
    $candidates = @()
    if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_NDK_HOME)) {
        $candidates += Join-Path $env:ANDROID_NDK_HOME "toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-nm.exe"
    }
    foreach ($name in @("llvm-nm", "nm")) {
        $command = Get-Command $name -ErrorAction SilentlyContinue
        if ($null -ne $command) {
            $candidates += $command.Source
        }
    }
    $nm = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($nm)) {
        Write-Warning "No nm tool found; exported symbols were not inspected."
        return $null
    }
    $output = & $nm --defined-only $Path 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Symbol inspection failed for $Path with exit code $LASTEXITCODE."
    }
    return $output | ForEach-Object { "$_" }
}

Push-Location $RepoRoot
try {
    $buildWindows = $Target -in @("Windows", "All")
    $buildAndroid = $Target -in @("Android", "All")

    if ($buildWindows) {
        Write-Host "== Native: Windows $Configuration =="
        Invoke-NativeStep "cargo build (windows)" {
            $cargoArgs = @("build", "-p", "nexus-ffi")
            if ($Configuration -eq "Release") {
                $cargoArgs += "--release"
            }
            if ($Locked) {
                $cargoArgs += "--locked"
            }
            & cargo @cargoArgs 2>&1
        }
        $windowsOut = Join-Path $NativeRoot "windows-x64"
        Copy-NativeArtifact `
            -Source (Join-Path $RepoRoot "target\$ProfileDir\nexus_ffi.dll") `
            -Destination (Join-Path $windowsOut "nexus_core.dll")

        # The GNU link tools regenerate the import library under the shipped
        # name and build the C harness. MSVC release builders do not ship them,
        # so their absence skips only these developer checks, never the artifact.
        $hasGnuLinkTools = $null -ne (Get-Command gendef -ErrorAction SilentlyContinue) -and
            $null -ne (Get-Command dlltool -ErrorAction SilentlyContinue)
        $hasGcc = $null -ne (Get-Command gcc -ErrorAction SilentlyContinue)
        if ($hasGnuLinkTools) {
            # The linker's import library records the DLL name compiled into the
            # artifact (`nexus_ffi.dll`). Regenerate it so the C harness and any
            # other native consumer link against the shipped `nexus_core.dll`.
            Push-Location $windowsOut
            try {
                Invoke-NativeStep "gendef (windows)" {
                    & gendef nexus_core.dll 2>&1 | Out-Null
                }
                $definition = Get-Content -LiteralPath (Join-Path $windowsOut "nexus_core.def") -Raw
                $definition = $definition -replace 'LIBRARY "nexus_ffi.dll"', 'LIBRARY "nexus_core.dll"'
                Set-Content -LiteralPath (Join-Path $windowsOut "nexus_core.def") -Value $definition -Encoding Ascii
                Invoke-NativeStep "dlltool (windows)" {
                    & dlltool -d nexus_core.def -l libnexus_core.dll.a 2>&1
                }
            } finally {
                Pop-Location
            }
        } else {
            Write-Warning "gendef/dlltool not found; skipped the renamed import library."
        }

        $symbols = Get-NativeSymbols -Path (Join-Path $windowsOut "nexus_core.dll")
        $required = @(
            "nexus_core_abi_version",
            "nexus_core_version",
            "nexus_core_create",
            "nexus_core_start",
            "nexus_core_stop",
            "nexus_core_destroy",
            "nexus_core_invoke",
            "nexus_core_next_event",
            "nexus_core_cancel",
            "nexus_core_free_buffer",
            "nexus_core_import_begin",
            "nexus_core_import_write",
            "nexus_core_import_finish",
            "nexus_core_import_abort",
            "nexus_core_export_open",
            "nexus_core_export_read",
            "nexus_core_export_close",
            "Java_io_nexus_core_client_NexusNative_nativeAbiVersion",
            "JNI_OnLoad"
        )
        if ($null -eq $symbols) {
            Write-Warning "Skipped Windows exported-symbol verification because no nm tool is available."
        } else {
            foreach ($name in $required) {
                if (-not ($symbols | Select-String -SimpleMatch $name -Quiet)) {
                    throw "Missing exported symbol: $name"
                }
            }
            Write-Host "  Exported symbols verified: $($required.Count) required entries present."
        }

        if (-not $SkipHarness -and $hasGcc -and $hasGnuLinkTools) {
            Write-Host "== Native: C harness =="
            $harnessSource = Join-Path $RepoRoot "native\c_harness\main.c"
            $harnessExe = Join-Path $windowsOut "nexus_c_harness.exe"
            Invoke-NativeStep "C harness compile" {
                & gcc -std=c11 -Wall -Wextra -Werror `
                    -I (Join-Path $RepoRoot "docs\architecture") `
                    $harnessSource `
                    (Join-Path $windowsOut "libnexus_core.dll.a") `
                    -o $harnessExe 2>&1
            }
            Invoke-NativeStep "C harness run" {
                & $harnessExe 2>&1
            }
        } elseif (-not $SkipHarness) {
            Write-Warning "gcc or GNU link tools not found; skipped the C harness."
        }
    }

    if ($buildAndroid) {
        Write-Host "== Native: Android $Configuration =="
        $ndk = $env:ANDROID_NDK_HOME
        if (-not (Test-Path -LiteralPath $ndk)) {
            throw "Android NDK not found at $ndk."
        }
        $ndkBin = Join-Path $ndk "toolchains\llvm\prebuilt\windows-x86_64\bin"
        $targets = @(
            @{
                Rust = "aarch64-linux-android"
                Abi = "arm64-v8a"
                Clang = "aarch64-linux-android28-clang.cmd"
                Cxx = "aarch64-linux-android28-clang++.cmd"
                EnvSuffix = "aarch64_linux_android"
                CargoLinker = "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
            },
            @{
                Rust = "x86_64-linux-android"
                Abi = "x86_64"
                Clang = "x86_64-linux-android28-clang.cmd"
                Cxx = "x86_64-linux-android28-clang++.cmd"
                EnvSuffix = "x86_64_linux_android"
                CargoLinker = "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"
            }
        )
        foreach ($androidTarget in $targets) {
            $linker = Join-Path $ndkBin $androidTarget.Clang
            if (-not (Test-Path -LiteralPath $linker)) {
                throw "Android linker not found: $linker"
            }
            Set-Item -Path "Env:$($androidTarget.CargoLinker)" -Value $linker
            Set-Item -Path "Env:CC_$($androidTarget.EnvSuffix)" -Value $linker
            Set-Item -Path "Env:CXX_$($androidTarget.EnvSuffix)" -Value (Join-Path $ndkBin $androidTarget.Cxx)
            Set-Item -Path "Env:AR_$($androidTarget.EnvSuffix)" -Value (Join-Path $ndkBin "llvm-ar.exe")
            Write-Host "  target: $($androidTarget.Rust)"
            Invoke-NativeStep "cargo build (android $($androidTarget.Rust))" {
                $cargoArgs = @("build", "-p", "nexus-ffi")
                if ($Configuration -eq "Release") {
                    $cargoArgs += "--release"
                }
                $cargoArgs += @("--target", $androidTarget.Rust)
                if ($Locked) {
                    $cargoArgs += "--locked"
                }
                & cargo @cargoArgs 2>&1
            }
            $androidOut = Join-Path $NativeRoot "android-$($androidTarget.Abi)"
            Copy-NativeArtifact `
                -Source (Join-Path $RepoRoot "target\$($androidTarget.Rust)\$ProfileDir\libnexus_ffi.so") `
                -Destination (Join-Path $androidOut "libnexus_core.so")
            Copy-NativeArtifact `
                -Source (Join-Path $RepoRoot "target\$($androidTarget.Rust)\$ProfileDir\libnexus_ffi.so") `
                -Destination (Join-Path $NativeRoot "android-jni\$($androidTarget.Abi)\libnexus_core.so")

            # Release Rust libraries embed debug information. Strip the packaged
            # copies so the APK carries only loadable code and symbols needed at
            # runtime; the build output under target/ keeps its full symbols.
            $strip = Join-Path $ndkBin "llvm-strip.exe"
            if (Test-Path -LiteralPath $strip) {
                foreach ($library in @(
                        (Join-Path $androidOut "libnexus_core.so"),
                        (Join-Path $NativeRoot "android-jni\$($androidTarget.Abi)\libnexus_core.so")
                    )) {
                    Invoke-NativeStep "llvm-strip ($($androidTarget.Abi))" {
                        & $strip --strip-debug $library 2>&1
                    }
                }
            } else {
                Write-Warning "llvm-strip not found; packaged Android libraries keep debug symbols."
            }

            $symbols = Get-NativeSymbols -Path (Join-Path $androidOut "libnexus_core.so")
            if ($null -eq $symbols) {
                Write-Warning "Skipped Android exported-symbol verification because no nm tool is available."
            } else {
                foreach ($name in @("nexus_core_abi_version", "nexus_core_create", "Java_io_nexus_core_client_NexusNative_nativeAbiVersion")) {
                    if (-not ($symbols | Select-String -SimpleMatch $name -Quiet)) {
                        throw "Missing exported Android symbol in $($androidTarget.Abi): $name"
                    }
                }
                Write-Host "  Android $($androidTarget.Abi) exported symbols verified."
            }
        }
    }

    Write-Host ""
    Write-Host "Native build complete."
} finally {
    Pop-Location
}
