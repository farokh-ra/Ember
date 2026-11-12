#Requires -Version 5.1
<#
.SYNOPSIS
    Clones the committed repository into a temporary directory and runs all checks.

.DESCRIPTION
    This proves that a clean checkout builds using only documented steps. The
    working tree must be committed first; uncommitted changes are not cloned.
#>
[CmdletBinding()]
param(
    [string]$Ref = "HEAD",
    [switch]$Keep
)

# Git and the nested build script may write progress to stderr. Windows
# PowerShell 5.1 treats stderr as terminating when this preference is Stop.
$ErrorActionPreference = "Continue"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$TempRoot = Join-Path $env:TEMP ("nexus-clean-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))

& git clone --quiet --local $RepoRoot $TempRoot 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "git clone failed."
}

& git -C $TempRoot checkout --quiet $Ref 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "git checkout $Ref failed."
}

Write-Host "Clean checkout: $TempRoot"
Push-Location $TempRoot
try {
    & (Join-Path $TempRoot "scripts\build.ps1") 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Clean checkout build failed."
    }
} finally {
    Pop-Location
}

if ($Keep) {
    Write-Host "Kept clean checkout at $TempRoot"
} else {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Removed clean checkout $TempRoot"
}
