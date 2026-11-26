#Requires -Version 5.1
<#
.SYNOPSIS
    Builds, signs, checksums, and manifests Ember release artifacts.

.DESCRIPTION
    Produces a per-user Windows MSI, a portable Windows ZIP, an Android APK, and
    an Android AAB under dist/<version>. Writing SHA256SUMS and
    release-manifest.json is part of the build, so published checksums always
    describe the exact bytes that were produced.

    Signing material is never read from the repository. Android signing uses
    EMBER_ANDROID_KEYSTORE, EMBER_ANDROID_STORE_PASSWORD,
    EMBER_ANDROID_KEY_ALIAS, and EMBER_ANDROID_KEY_PASSWORD. Windows signing
    uses EMBER_WINDOWS_PFX plus EMBER_WINDOWS_PFX_PASSWORD, or an
    EMBER_WINDOWS_CERT_THUMBPRINT already present in the certificate store.

    Use -AllowUnsigned only for a local packaging dry run. A release requires
    signatures.
#>
[CmdletBinding()]
param(
    [string]$Version = "0.1.0",
    [string]$OutputDir = "dist",
    [switch]$SkipWindows,
    [switch]$SkipAndroid,
    [switch]$SkipNative,
    [switch]$AllowUnsigned,
    [switch]$AllowDirty,
    [string]$WindowsPfxPath = $env:EMBER_WINDOWS_PFX,
    [string]$WindowsPfxPassword = $env:EMBER_WINDOWS_PFX_PASSWORD,
    [string]$WindowsCertificateThumbprint = $env:EMBER_WINDOWS_CERT_THUMBPRINT,
    [string]$TimestampUrl = $env:EMBER_TIMESTAMP_URL
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "dev-env.ps1") -Quiet

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

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $utf8)
}

function Get-FileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Add-Artifact {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][System.Collections.ArrayList]$List,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Kind,
        [bool]$Signed = $false
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Expected release artifact is missing: $Path"
    }
    $item = Get-Item -LiteralPath $Path
    [void]$List.Add([ordered]@{
            name = $item.Name
            kind = $Kind
            bytes = $item.Length
            sha256 = Get-FileSha256 -Path $item.FullName
            signed = $Signed
        })
}

function Test-AndroidSigning {
    $required = @(
        $env:EMBER_ANDROID_KEYSTORE,
        $env:EMBER_ANDROID_STORE_PASSWORD,
        $env:EMBER_ANDROID_KEY_ALIAS,
        $env:EMBER_ANDROID_KEY_PASSWORD
    )
    return @($required | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -eq 0
}

function Invoke-WindowsSigning {
    param(
        [Parameter(Mandatory = $true)][string[]]$Files
    )

    $existing = @($Files | Where-Object { Test-Path -LiteralPath $_ })
    if ($existing.Count -eq 0) {
        return $false
    }

    if (-not [string]::IsNullOrWhiteSpace($WindowsPfxPath)) {
        $signTool = Get-Command signtool.exe -ErrorAction SilentlyContinue
        if ($null -eq $signTool) {
            throw "signtool.exe is required to sign with EMBER_WINDOWS_PFX."
        }
        foreach ($file in $existing) {
            $wasReadOnly = (Get-Item -LiteralPath $file).IsReadOnly
            if ($wasReadOnly) {
                Set-ItemProperty -LiteralPath $file -Name IsReadOnly -Value $false
            }
            try {
                $arguments = @("sign", "/fd", "SHA256")
                if (-not [string]::IsNullOrWhiteSpace($TimestampUrl)) {
                    $arguments += @("/tr", $TimestampUrl, "/td", "SHA256")
                }
                $arguments += @("/f", $WindowsPfxPath, "/p", $WindowsPfxPassword, $file)
                Invoke-NativeStep "signtool ($file)" { & $signTool.Source @arguments 2>&1 }
            } finally {
                if ($wasReadOnly) {
                    Set-ItemProperty -LiteralPath $file -Name IsReadOnly -Value $true
                }
            }
        }
        return $true
    }

    if (-not [string]::IsNullOrWhiteSpace($WindowsCertificateThumbprint)) {
        $certificate = @()
        foreach ($store in @("Cert:\CurrentUser\My", "Cert:\LocalMachine\My")) {
            $certificate += Get-ChildItem $store -ErrorAction SilentlyContinue |
                Where-Object { $_.Thumbprint -eq $WindowsCertificateThumbprint }
        }
        $certificate = $certificate | Select-Object -First 1
        if ($null -eq $certificate) {
            throw "No code-signing certificate with thumbprint $WindowsCertificateThumbprint was found."
        }
        foreach ($file in $existing) {
            $wasReadOnly = (Get-Item -LiteralPath $file).IsReadOnly
            if ($wasReadOnly) {
                Set-ItemProperty -LiteralPath $file -Name IsReadOnly -Value $false
            }
            try {
                $signatureArgs = @{
                    FilePath = $file
                    Certificate = $certificate
                    HashAlgorithm = "SHA256"
                }
                if (-not [string]::IsNullOrWhiteSpace($TimestampUrl)) {
                    $signatureArgs.TimestampServer = $TimestampUrl
                }
                $result = Set-AuthenticodeSignature @signatureArgs
                if ($null -eq $result.SignerCertificate) {
                    throw "Signing $file failed: $($result.StatusMessage)"
                }
                Write-Host "  signed $file ($($result.Status))"
            } finally {
                if ($wasReadOnly) {
                    Set-ItemProperty -LiteralPath $file -Name IsReadOnly -Value $true
                }
            }
        }
        return $true
    }

    return $false
}

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version must use MAJOR.MINOR.PATCH, got '$Version'."
}
$versionParts = $Version.Split(".")
$versionCode = [int]$versionParts[0] * 10000 + [int]$versionParts[1] * 100 + [int]$versionParts[2]
if ([int]$versionParts[0] -gt 255 -or [int]$versionParts[1] -gt 255 -or [int]$versionParts[2] -gt 65535) {
    throw "Windows MSI requires major and minor at most 255 and build at most 65535, got '$Version'."
}

Push-Location $RepoRoot
try {
    if (-not $AllowDirty) {
        $dirty = & git status --porcelain
        if ($LASTEXITCODE -ne 0) {
            throw "git status failed with exit code $LASTEXITCODE."
        }
        if ($dirty) {
            throw "The working tree is not clean. Commit the release inputs or pass -AllowDirty for a local dry run."
        }
    }

    $releaseRoot = Join-Path $RepoRoot $OutputDir
    $releaseDir = Join-Path $releaseRoot $Version
    if (Test-Path -LiteralPath $releaseDir) {
        Remove-Item -LiteralPath $releaseDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

    if (-not $SkipNative) {
        Write-Host "== Native: Windows and Android release libraries =="
        Invoke-NativeStep "native release build" {
            & (Join-Path $PSScriptRoot "build-native.ps1") -Target All -Configuration Release -Locked 2>&1
        }
    }

    if (-not $SkipWindows) {
        $wixDir = & (Join-Path $PSScriptRoot "fetch-wix.ps1")
        $env:PATH = "$wixDir;$env:PATH"
    }

    $artifacts = New-Object System.Collections.ArrayList
    $androidSigned = $false
    $windowsSigned = $false

    if (-not $SkipAndroid) {
        $releaseSigningReady = Test-AndroidSigning
        if (-not $releaseSigningReady -and -not $AllowUnsigned) {
            throw "Android release signing is not configured. Set the EMBER_ANDROID_* variables or pass -AllowUnsigned for a dry run."
        }
        Write-Host "== Gradle: Android release APK and AAB =="
        Invoke-NativeStep "Gradle Android release" {
            & .\gradlew.bat --no-daemon --console=plain `
                :apps:androidApp:assembleRelease `
                :apps:androidApp:bundleRelease `
                "-PemberVersion=$Version" `
                "-PemberVersionCode=$versionCode" 2>&1
        }

        $apkSource = Get-ChildItem (Join-Path $RepoRoot "apps\androidApp\build\outputs\apk\release") -Filter "*.apk" |
            Sort-Object Name |
            Select-Object -First 1
        $aabSource = Get-ChildItem (Join-Path $RepoRoot "apps\androidApp\build\outputs\bundle\release") -Filter "*.aab" |
            Select-Object -First 1
        if ($null -eq $apkSource -or $null -eq $aabSource) {
            throw "Android release outputs were not produced."
        }
        $apkTarget = Join-Path $releaseDir "Ember-$Version-android.apk"
        $aabTarget = Join-Path $releaseDir "Ember-$Version-android.aab"
        Copy-Item -LiteralPath $apkSource.FullName -Destination $apkTarget -Force -ErrorAction Stop
        Copy-Item -LiteralPath $aabSource.FullName -Destination $aabTarget -Force -ErrorAction Stop
        $androidSigned = $releaseSigningReady
        Add-Artifact -List $artifacts -Path $apkTarget -Kind "android-apk" -Signed $androidSigned
        Add-Artifact -List $artifacts -Path $aabTarget -Kind "android-aab" -Signed $androidSigned
    }

    if (-not $SkipWindows) {
        $nativeDll = Join-Path $RepoRoot "build\native\windows-x64\nexus_core.dll"
        if (-not (Test-Path -LiteralPath $nativeDll)) {
            throw "Windows native core not found at $nativeDll. Build it or drop -SkipNative."
        }

        Write-Host "== Gradle: Windows app image and per-user MSI =="
        Invoke-NativeStep "Gradle Windows packages" {
            & .\gradlew.bat --no-daemon --console=plain `
                :apps:desktopApp:createDistributable `
                :apps:desktopApp:packageMsi `
                "-PemberVersion=$Version" 2>&1
        }

        $appImage = Join-Path $RepoRoot "apps\desktopApp\build\compose\binaries\main\app\Ember"
        $packagedDll = Join-Path $appImage "app\resources\nexus_core.dll"
        if (-not (Test-Path -LiteralPath $packagedDll)) {
            throw "The app image does not contain nexus_core.dll; check appResourcesRootDir packaging."
        }

        $exePath = Join-Path $appImage "Ember.exe"
        $msiSource = Join-Path $RepoRoot "apps\desktopApp\build\compose\binaries\main\msi\Ember-$Version.msi"
        $msiTarget = Join-Path $releaseDir "Ember-$Version-windows.msi"
        $zipTarget = Join-Path $releaseDir "Ember-$Version-windows-portable.zip"

        $windowsSigned = Invoke-WindowsSigning -Files @($exePath, $msiSource)
        if (-not $windowsSigned -and -not $AllowUnsigned) {
            throw "Windows signing is not configured. Set EMBER_WINDOWS_PFX or EMBER_WINDOWS_CERT_THUMBPRINT, or pass -AllowUnsigned for a dry run."
        }

        Copy-Item -LiteralPath $msiSource -Destination $msiTarget -Force
        if (Test-Path -LiteralPath $zipTarget) {
            Remove-Item -LiteralPath $zipTarget -Force
        }
        Write-Host "  compressing portable Windows package"
        Compress-Archive -LiteralPath $appImage -DestinationPath $zipTarget -CompressionLevel Optimal

        Add-Artifact -List $artifacts -Path $msiTarget -Kind "windows-msi" -Signed $windowsSigned
        # The ZIP is a container; its executable payload is signed, but the
        # container itself carries no signature.
        Add-Artifact -List $artifacts -Path $zipTarget -Kind "windows-portable-zip" -Signed $false
    }

    $checksumLines = @()
    foreach ($artifact in $artifacts) {
        $checksumLines += "$($artifact.sha256)  $($artifact.name)"
    }
    Write-Utf8NoBom -Path (Join-Path $releaseDir "SHA256SUMS") -Content (($checksumLines -join "`n") + "`n")

    $commit = (& git rev-parse HEAD).Trim()
    $branch = (& git branch --show-current).Trim()
    $rustc = (& rustc --version).Trim()
    $cargo = (& cargo --version).Trim()
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $javaRaw = (& "$env:JAVA_HOME\bin\java.exe" -version 2>&1 | Select-Object -First 1)
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    $java = "$javaRaw".Trim()
    $wrapperProperties = Get-Content -LiteralPath (Join-Path $RepoRoot "gradle\wrapper\gradle-wrapper.properties") -Raw
    $gradleMatch = [regex]::Match($wrapperProperties, 'gradle-([0-9.]+)-bin\.zip')
    $gradle = if ($gradleMatch.Success) { $gradleMatch.Groups[1].Value } else { "unknown" }

    $manifest = [ordered]@{
        schemaVersion = 1
        product = "Ember"
        version = $Version
        versionCode = $versionCode
        commit = $commit
        branch = $branch
        builtAtUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platform = [Environment]::OSVersion.VersionString
        toolchain = [ordered]@{
            rustc = $rustc
            cargo = $cargo
            java = $java
            gradle = $gradle
        }
        signing = [ordered]@{
            android = $androidSigned
            windows = $windowsSigned
        }
        artifacts = $artifacts
        notes = "Checksums describe the exact artifact bytes. See docs/release/ for verification and signing policy."
    }

    $manifestJson = $manifest | ConvertTo-Json -Depth 6
    Write-Utf8NoBom -Path (Join-Path $releaseDir "release-manifest.json") -Content ($manifestJson + "`n")

    Write-Host ""
    Write-Host "== Verifying release directory =="
    & (Join-Path $PSScriptRoot "verify-release.ps1") -Directory $releaseDir -RequireSigned:(-not $AllowUnsigned)
    if ($LASTEXITCODE -ne 0) {
        throw "Release verification failed with exit code $LASTEXITCODE."
    }

    Write-Host ""
    Write-Host "Release artifacts written to $releaseDir"
    foreach ($artifact in $artifacts) {
        Write-Host ("  {0} ({1:N0} bytes)" -f $artifact.name, $artifact.bytes)
    }
} finally {
    Pop-Location
}
