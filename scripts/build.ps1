#Requires -Version 5.1
<#
.SYNOPSIS
    Runs the local Ember build, test, formatting, lint, and hygiene gates.
#>
[CmdletBinding()]
param(
    [switch]$SkipGradle,
    [switch]$SkipRust,
    [switch]$SkipNative,
    [switch]$SkipHygiene,
    [switch]$WithDependencyScan
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

. (Join-Path $PSScriptRoot "dev-env.ps1") -Quiet

<#
Windows PowerShell 5.1 turns any stderr output from a native command into a
terminating error when $ErrorActionPreference is Stop, even when the command
succeeds. Run native tools with Continue and check $LASTEXITCODE explicitly.
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

Push-Location $RepoRoot
try {
    if (-not $SkipNative) {
        Write-Host "== Native: Windows and Android artifacts, C harness =="
        & (Join-Path $PSScriptRoot "build-native.ps1") -Target All -Configuration Release
        if ($LASTEXITCODE -ne 0) {
            throw "native build failed with exit code $LASTEXITCODE."
        }
    }

    if (-not $SkipGradle) {
        Write-Host "== Gradle: format, static analysis, builds, tests =="
        Invoke-NativeStep "Gradle checks" {
            & .\gradlew.bat --no-daemon --console=plain `
                spotlessCheck `
                detekt `
                :apps:androidApp:assembleDebug `
                :apps:desktopApp:createDistributable `
                :shared:model:jvmTest `
                :shared:core-api:jvmTest `
                :shared:core-client:jvmTest `
                :shared:presentation:jvmTest `
                :shared:ui:jvmTest 2>&1
        }
    }

    if (-not $SkipRust) {
        Write-Host "== Rust: format, lint, tests =="
        Invoke-NativeStep "cargo fmt" {
            & cargo fmt --all -- --check 2>&1
        }

        Invoke-NativeStep "cargo clippy" {
            & cargo clippy --workspace --all-targets -- -D warnings 2>&1
        }

        Invoke-NativeStep "cargo test" {
            & cargo test --workspace 2>&1
        }

        if ($WithDependencyScan) {
            Write-Host "== Rust: dependency and advisory policy =="
            Invoke-NativeStep "cargo deny" {
                & cargo deny check 2>&1
            }

            Invoke-NativeStep "cargo audit" {
                & cargo audit 2>&1
            }

            Write-Host "== Supply chain: CycloneDX SBOM =="
            Invoke-NativeStep "SBOM generation" {
                & python (Join-Path $PSScriptRoot "generate_sbom.py") `
                    --version "0.1.0" `
                    --sbom (Join-Path $RepoRoot "dist\sbom\ember-0.1.0.cdx.json") 2>&1
            }
        }
    }

    if (-not $SkipHygiene) {
        Write-Host "== Repository hygiene =="
        Invoke-NativeStep "repository hygiene" {
            & python (Join-Path $PSScriptRoot "check_repo_hygiene.py") 2>&1
        }
    }

    Write-Host ""
    Write-Host "All requested local checks passed."
} finally {
    Pop-Location
}
