#Requires -Version 5.1
<#
.SYNOPSIS
    Verifies a directory produced by package-release.ps1.

.DESCRIPTION
    Checks artifact sizes and SHA-256 hashes against release-manifest.json,
    re-reads SHA256SUMS, and inspects Android and Windows signatures. With
    -RequireSigned, every platform artifact must carry a signature. A local
    self-signed certificate is allowed here; production trust is a separate
    release-checklist item.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Directory,
    [switch]$RequireSigned
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Directory = (Resolve-Path -LiteralPath $Directory).Path
$failures = New-Object System.Collections.ArrayList

function Add-Failure {
    param([Parameter(Mandatory = $true)][string]$Message)
    [void]$failures.Add($Message)
    Write-Host "FAIL: $Message"
}

function Test-NativeOutput {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$Step
    )

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Action 2>&1 | ForEach-Object { "$_" }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    return [pscustomobject]@{ Output = $output; ExitCode = $exitCode; Step = $Step }
}

$manifestPath = Join-Path $Directory "release-manifest.json"
$checksumsPath = Join-Path $Directory "SHA256SUMS"
if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "release-manifest.json is missing from $Directory."
}
if (-not (Test-Path -LiteralPath $checksumsPath)) {
    throw "SHA256SUMS is missing from $Directory."
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.product -ne "Ember") {
    Add-Failure "Manifest product is '$($manifest.product)', expected Ember."
}
if ([string]::IsNullOrWhiteSpace($manifest.version)) {
    Add-Failure "Manifest version is empty."
}
if ((Split-Path -Leaf $Directory) -ne $manifest.version) {
    Add-Failure "Directory name does not match manifest version $($manifest.version)."
}

$artifacts = @($manifest.artifacts)
if ($artifacts.Count -eq 0) {
    Add-Failure "Manifest contains no artifacts."
}

$checksumMap = @{}
foreach ($line in Get-Content -LiteralPath $checksumsPath) {
    if ([string]::IsNullOrWhiteSpace($line)) {
        continue
    }
    $match = [regex]::Match($line, '^([0-9a-fA-F]{64})\s{2}(.+)$')
    if (-not $match.Success) {
        Add-Failure "Malformed SHA256SUMS line: $line"
        continue
    }
    $checksumMap[$match.Groups[2].Value] = $match.Groups[1].Value.ToLowerInvariant()
}

foreach ($artifact in $artifacts) {
    $path = Join-Path $Directory $artifact.name
    if (-not (Test-Path -LiteralPath $path)) {
        Add-Failure "Missing artifact $($artifact.name)."
        continue
    }
    $item = Get-Item -LiteralPath $path
    if ($item.Length -ne [long]$artifact.bytes) {
        Add-Failure "Size mismatch for $($artifact.name): manifest $($artifact.bytes), file $($item.Length)."
    }
    $actualHash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne "$($artifact.sha256)".ToLowerInvariant()) {
        Add-Failure "Hash mismatch for $($artifact.name)."
    }
    if (-not $checksumMap.ContainsKey($artifact.name)) {
        Add-Failure "SHA256SUMS does not list $($artifact.name)."
    } elseif ($checksumMap[$artifact.name] -ne $actualHash) {
        Add-Failure "SHA256SUMS hash mismatch for $($artifact.name)."
    }
}

$extraChecksums = @($checksumMap.Keys | Where-Object { $_ -notin @($artifacts | ForEach-Object { $_.name }) })
if ($extraChecksums.Count -gt 0) {
    Add-Failure "SHA256SUMS lists files that are not artifacts: $($extraChecksums -join ', ')."
}

$sbom = @($artifacts | Where-Object { $_.kind -eq "cyclonedx-sbom" }) | Select-Object -First 1
if ($null -eq $sbom) {
    if ($RequireSigned) {
        Add-Failure "Manifest has no CycloneDX SBOM artifact."
    }
} else {
    $sbomPath = Join-Path $Directory $sbom.name
    try {
        $sbomJson = Get-Content -LiteralPath $sbomPath -Raw | ConvertFrom-Json
        if ($sbomJson.bomFormat -ne "CycloneDX" -or $sbomJson.specVersion -ne "1.5") {
            Add-Failure "$($sbom.name) is not a CycloneDX 1.5 document."
        } elseif (@($sbomJson.components).Count -lt 100) {
            Add-Failure "$($sbom.name) has an implausibly small component list."
        } else {
            Write-Host "SBOM verified: $($sbom.name) ($(@($sbomJson.components).Count) components)"
        }
    } catch {
        Add-Failure "$($sbom.name) is not valid JSON: $($_.Exception.Message)"
    }
}

$noticeBundle = @($artifacts | Where-Object { $_.kind -eq "third-party-notices" }) | Select-Object -First 1
if ($null -eq $noticeBundle) {
    if ($RequireSigned) {
        Add-Failure "Manifest has no third-party notice bundle."
    }
} else {
    $noticePath = Join-Path $Directory $noticeBundle.name
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $noticeArchive = [System.IO.Compression.ZipFile]::OpenRead($noticePath)
    try {
        $noticeNames = @($noticeArchive.Entries | ForEach-Object { $_.FullName })
    } finally {
        $noticeArchive.Dispose()
    }
    if (-not ($noticeNames | Where-Object { $_ -match "THIRD_PARTY_NOTICES\.md$" })) {
        Add-Failure "$($noticeBundle.name) does not contain THIRD_PARTY_NOTICES.md."
    } elseif (-not ($noticeNames | Where-Object { $_ -match "license-files\.json$" })) {
        Add-Failure "$($noticeBundle.name) does not contain license-files.json."
    } else {
        Write-Host "Third-party notice bundle verified: $($noticeBundle.name)"
    }
}

$apk = @($artifacts | Where-Object { $_.kind -eq "android-apk" }) | Select-Object -First 1
if ($null -ne $apk) {
    $apkPath = Join-Path $Directory $apk.name
    if (-not $apk.signed -and -not $RequireSigned) {
        Write-Host "APK is unsigned (local dry run): $($apk.name)"
    } else {
        $androidSdk = $env:ANDROID_HOME
        if ([string]::IsNullOrWhiteSpace($androidSdk)) {
            $androidSdk = $env:ANDROID_SDK_ROOT
        }
        $apksigner = $null
        if (-not [string]::IsNullOrWhiteSpace($androidSdk)) {
            $apksigner = Get-ChildItem (Join-Path $androidSdk "build-tools") -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending |
                ForEach-Object { Join-Path $_.FullName "apksigner.bat" } |
                Where-Object { Test-Path -LiteralPath $_ } |
                Select-Object -First 1
        }
        if ($null -eq $apksigner) {
            $apksignerCommand = Get-Command apksigner.bat -ErrorAction SilentlyContinue
            if ($null -ne $apksignerCommand) {
                $apksigner = $apksignerCommand.Source
            }
        }
        if ($null -eq $apksigner) {
            if ($RequireSigned) {
                Add-Failure "apksigner was not found, so $($apk.name) could not be verified."
            }
        } else {
            $result = Test-NativeOutput -Step "apksigner" -Action { & cmd.exe /c "`"$apksigner`" verify --verbose --print-certs `"$apkPath`"" }
            $apkText = $result.Output -join "`n"
            if ($apkText -notmatch "Verifies") {
                Add-Failure "apksigner did not report a verified APK for $($apk.name)."
            } elseif ($result.ExitCode -ne 0) {
                Add-Failure "apksigner exited with code $($result.ExitCode) for $($apk.name)."
            } else {
                Write-Host "APK signature verified: $($apk.name)"
            }
            if ($RequireSigned -and $apkText -match "Number of signers:\s+0") {
                Add-Failure "$($apk.name) has no signer."
            }
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            $apkArchive = [System.IO.Compression.ZipFile]::OpenRead($apkPath)
            try {
                $apkEntries = @($apkArchive.Entries | ForEach-Object { $_.FullName })
            } finally {
                $apkArchive.Dispose()
            }
            foreach ($abi in @("arm64-v8a", "x86_64")) {
                if (-not ($apkEntries -contains "lib/$abi/libnexus_core.so")) {
                    Add-Failure "$($apk.name) is missing lib/$abi/libnexus_core.so."
                }
            }
        }
    }
}

$aab = @($artifacts | Where-Object { $_.kind -eq "android-aab" }) | Select-Object -First 1
if ($null -ne $aab) {
    $aabPath = Join-Path $Directory $aab.name
    if (-not $aab.signed -and -not $RequireSigned) {
        Write-Host "AAB is unsigned (local dry run): $($aab.name)"
    } else {
        $jarsigner = $null
        if (-not [string]::IsNullOrWhiteSpace($env:JAVA_HOME)) {
            $candidate = Join-Path $env:JAVA_HOME "bin\jarsigner.exe"
            if (Test-Path -LiteralPath $candidate) {
                $jarsigner = $candidate
            }
        }
        if ($null -eq $jarsigner) {
            $jarsignerCommand = Get-Command jarsigner.exe -ErrorAction SilentlyContinue
            if ($null -ne $jarsignerCommand) {
                $jarsigner = $jarsignerCommand.Source
            }
        }
        if ($null -ne $jarsigner -and (Test-Path -LiteralPath $jarsigner)) {
            $result = Test-NativeOutput -Step "jarsigner" -Action { & $jarsigner -verify $aabPath }
            $aabText = $result.Output -join "`n"
            if ($aabText -match "(?m)^jar is unsigned|no manifest") {
                Add-Failure "jarsigner reported an unsigned AAB for $($aab.name)."
            } elseif ($aabText -notmatch "jar verified") {
                Add-Failure "jarsigner did not report a verified AAB for $($aab.name)."
            } else {
                # jarsigner exits non-zero for self-signed or expiring warnings.
                Write-Host "AAB signature verified: $($aab.name)"
            }
        } elseif ($RequireSigned) {
            Add-Failure "jarsigner was not found, so $($aab.name) could not be verified."
        }
    }
}

$msi = @($artifacts | Where-Object { $_.kind -eq "windows-msi" }) | Select-Object -First 1
if ($null -ne $msi) {
    $msiPath = Join-Path $Directory $msi.name
    if (-not $msi.signed -and -not $RequireSigned) {
        Write-Host "MSI is unsigned (local dry run): $($msi.name)"
    } else {
        $signature = Get-AuthenticodeSignature -FilePath $msiPath
        if ($null -ne $signature.SignerCertificate) {
            if ($signature.Status -eq "Valid") {
                Write-Host "MSI Authenticode signature is valid: $($msi.name)"
            } else {
                Write-Host "MSI signature present ($($signature.Status)); production requires a trusted chain: $($msi.name)"
            }
        } else {
            Add-Failure "$($msi.name) is not Authenticode signed."
        }
    }
}

$zip = @($artifacts | Where-Object { $_.kind -eq "windows-portable-zip" }) | Select-Object -First 1
if ($null -ne $zip) {
    $zipPath = Join-Path $Directory $zip.name
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $names = @($archive.Entries | ForEach-Object { $_.FullName })
    } finally {
        $archive.Dispose()
    }
    if (-not ($names | Where-Object { $_ -match "(^|[\\/])nexus_core\.dll$" })) {
        Add-Failure "$($zip.name) does not contain nexus_core.dll."
    } else {
        Write-Host "Portable package contains the native core: $($zip.name)"
    }
    if (-not ($names | Where-Object { $_ -match "(^|[\\/])Ember\.exe$" })) {
        Add-Failure "$($zip.name) does not contain Ember.exe."
    }
    if (-not ($names | Where-Object { $_ -match "(^|[\\/])THIRD_PARTY_NOTICES\.md$" })) {
        Add-Failure "$($zip.name) does not contain THIRD_PARTY_NOTICES.md."
    }
}

if ($RequireSigned) {
    $signableKinds = @("android-apk", "android-aab", "windows-msi")
    foreach ($artifact in $artifacts) {
        if ($artifact.kind -in $signableKinds -and -not $artifact.signed) {
            Add-Failure "$($artifact.name) is marked unsigned in the manifest."
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host ""
    Write-Host "RELEASE VERIFICATION: FAIL ($($failures.Count) issue(s))"
    exit 1
}

Write-Host ""
Write-Host "RELEASE VERIFICATION: PASS"
exit 0
