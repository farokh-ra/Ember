#Requires -Version 5.1
<#
.SYNOPSIS
    Downloads the pinned WiX 3 toolset used by the Windows MSI build.

.DESCRIPTION
    Compose Desktop and jpackage require WiX 3 (candle/light) to produce an
    MSI. The toolset is placed under the gitignored NeededToolsHere directory so
    no binary is committed. The archive hash is pinned and verified before it is
    expanded.
#>
[CmdletBinding()]
param(
    [string]$Destination = (Join-Path $PSScriptRoot "..\NeededToolsHere\wix314"),
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$Version = "3.14.1.8722"
$Url = "https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip"
$Sha256 = "6AC824E1642D6F7277D0ED7EA09411A508F6116BA6FAE0AA5F2C7DAA2FF43D31"

$Destination = [IO.Path]::GetFullPath($Destination)
$Archive = Join-Path (Split-Path $Destination -Parent) "wix314-binaries.zip"

if ((Test-Path -LiteralPath (Join-Path $Destination "candle.exe")) -and -not $Force) {
    Write-Host "WiX $Version already present at $Destination"
    Write-Output $Destination
    return
}

New-Item -ItemType Directory -Force -Path (Split-Path $Destination -Parent) | Out-Null

if (-not (Test-Path -LiteralPath $Archive)) {
    Write-Host "Downloading WiX $Version"
    Invoke-WebRequest -Uri $Url -OutFile $Archive -UseBasicParsing
}

$actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash
if ($actual -ne $Sha256) {
    throw "WiX archive hash mismatch. Expected $Sha256, got $actual."
}

if (Test-Path -LiteralPath $Destination) {
    Remove-Item -LiteralPath $Destination -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Expand-Archive -LiteralPath $Archive -DestinationPath $Destination -Force

if (-not (Test-Path -LiteralPath (Join-Path $Destination "candle.exe"))) {
    throw "WiX extraction did not produce candle.exe in $Destination."
}

Write-Host "WiX $Version ready at $Destination"
Write-Output $Destination
