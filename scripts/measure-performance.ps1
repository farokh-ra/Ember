#Requires -Version 5.1
<#
.SYNOPSIS
    Measures Ember desktop startup, idle memory, idle CPU, and package sizes.

.DESCRIPTION
    Launches the jpackage application image, waits for the Java child process to
    create its main window, samples memory and CPU, and writes a JSON report.
    The measurement is intentionally local and deterministic: it reports the
    machine, run count, and soak duration alongside the numbers.

    Close any running Ember instance first. The script fails instead of
    terminating an application the operator may be using.
#>
[CmdletBinding()]
param(
    [string]$AppPath = "",
    [ValidateRange(1, 10)][int]$Runs = 3,
    [ValidateRange(0, 600)][int]$SoakSeconds = 30,
    [string]$JsonPath = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if ([string]::IsNullOrWhiteSpace($AppPath)) {
    $AppPath = Join-Path $RepoRoot "apps\desktopApp\build\compose\binaries\main\app\Ember\Ember.exe"
}
$AppPath = [IO.Path]::GetFullPath($AppPath)
if (-not (Test-Path -LiteralPath $AppPath)) {
    throw "App image not found at $AppPath. Run .\gradlew.bat :apps:desktopApp:createDistributable first."
}

$running = @(Get-Process Ember -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    throw "Close the running Ember process before measuring. Nothing was terminated."
}

function Stop-EmberProcesses {
    param([int[]]$Ids)
    foreach ($id in $Ids) {
        Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    }
}

function Get-DirectorySize {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return $null
    }
    return (Get-ChildItem -LiteralPath $Path -Recurse -File | Measure-Object -Property Length -Sum).Sum
}

$startupSamples = @()
$memorySamples = @()
$soak = $null

for ($run = 1; $run -le $Runs; $run++) {
    Write-Host "Run $run of $Runs"
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $parent = Start-Process -FilePath $AppPath -PassThru
    $child = $null
    try {
        while ($watch.Elapsed.TotalSeconds -lt 30) {
            Start-Sleep -Milliseconds 100
            if ($parent.HasExited) {
                throw "The Ember launcher exited before a window appeared."
            }
            $child = Get-Process Ember -ErrorAction SilentlyContinue |
                Where-Object { $_.Id -ne $parent.Id -and $_.MainWindowHandle -ne 0 } |
                Select-Object -First 1
            if ($null -ne $child) {
                break
            }
        }
        if ($null -eq $child) {
            throw "No Ember window appeared within 30 seconds."
        }
        $watch.Stop()
        $startupMs = [int]$watch.Elapsed.TotalMilliseconds
        $startupSamples += $startupMs
        Write-Host "  startup: $startupMs ms"

        Start-Sleep -Seconds 5
        $child.Refresh()
        $ram = $child.WorkingSet64
        $memorySamples += $ram
        Write-Host "  working set: $([math]::Round($ram / 1MB, 1)) MiB"

        if ($run -eq $Runs -and $SoakSeconds -gt 0) {
            $cpuBefore = $child.TotalProcessorTime.TotalMilliseconds
            $ramBefore = $child.WorkingSet64
            Write-Host "  soaking for $SoakSeconds s"
            Start-Sleep -Seconds $SoakSeconds
            $child.Refresh()
            $cpuDelta = $child.TotalProcessorTime.TotalMilliseconds - $cpuBefore
            $ramAfter = $child.WorkingSet64
            $soak = [ordered]@{
                seconds = $SoakSeconds
                cpuPercentOfOneCore = [math]::Round(($cpuDelta / ($SoakSeconds * 1000)) * 100, 3)
                cpuPercentOfMachine = [math]::Round((($cpuDelta / ($SoakSeconds * 1000)) * 100) / [Environment]::ProcessorCount, 3)
                workingSetStartBytes = $ramBefore
                workingSetEndBytes = $ramAfter
                workingSetGrowthBytes = $ramAfter - $ramBefore
            }
            Write-Host "  idle CPU: $($soak.cpuPercentOfMachine)% of machine; memory growth $([math]::Round($soak.workingSetGrowthBytes / 1MB, 1)) MiB"
        }
    } finally {
        $ids = @()
        if ($null -ne $child) { $ids += $child.Id }
        if ($null -ne $parent -and -not $parent.HasExited) { $ids += $parent.Id }
        Stop-EmberProcesses -Ids $ids
        Start-Sleep -Milliseconds 500
    }
}

function Get-Median {
    param([double[]]$Values)
    $sorted = $Values | Sort-Object
    $middle = [int][math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 1) {
        return $sorted[$middle]
    }
    return ($sorted[$middle - 1] + $sorted[$middle]) / 2
}

$appDir = Split-Path -Parent $AppPath
$msi = Get-ChildItem (Join-Path $RepoRoot "apps\desktopApp\build\compose\binaries\main\msi") -Filter "Ember-*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
$apk = Get-ChildItem (Join-Path $RepoRoot "apps\androidApp\build\outputs\apk\release") -Filter "*.apk" -ErrorAction SilentlyContinue |
    Select-Object -First 1
$aab = Get-ChildItem (Join-Path $RepoRoot "apps\androidApp\build\outputs\bundle\release") -Filter "*.aab" -ErrorAction SilentlyContinue |
    Select-Object -First 1

$appImageBytes = Get-DirectorySize -Path $appDir
$msiBytes = if ($null -ne $msi) { [long]$msi.Length } else { $null }
$apkBytes = if ($null -ne $apk) { [long]$apk.Length } else { $null }
$aabBytes = if ($null -ne $aab) { [long]$aab.Length } else { $null }

$report = [ordered]@{
    schemaVersion = 1
    measuredAtUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    app = [ordered]@{
        path = $AppPath
        imageBytes = $appImageBytes
        msiBytes = $msiBytes
        androidApkBytes = $apkBytes
        androidAabBytes = $aabBytes
    }
    machine = [ordered]@{
        os = (Get-CimInstance Win32_OperatingSystem).Caption
        version = [Environment]::OSVersion.VersionString
        cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
        logicalProcessors = [Environment]::ProcessorCount
        totalMemoryBytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
    }
    method = [ordered]@{
        runs = $Runs
        settleSecondsBeforeMemory = 5
        soakSeconds = $SoakSeconds
        startupDefinition = "launcher start until the Java child process reports a main window"
        idleDefinition = "window open and untouched; total process CPU over the soak window"
    }
    startupMs = [ordered]@{
        samples = $startupSamples
        median = Get-Median -Values $startupSamples
        minimum = ($startupSamples | Measure-Object -Minimum).Minimum
        maximum = ($startupSamples | Measure-Object -Maximum).Maximum
    }
    workingSetBytes = [ordered]@{
        samples = $memorySamples
        median = Get-Median -Values $memorySamples
    }
    soak = $soak
}

$json = $report | ConvertTo-Json -Depth 6
if ([string]::IsNullOrWhiteSpace($JsonPath)) {
    $JsonPath = Join-Path $RepoRoot "dist\performance.json"
}
$JsonPath = [IO.Path]::GetFullPath($JsonPath)
New-Item -ItemType Directory -Force -Path (Split-Path $JsonPath -Parent) | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($JsonPath, $json + "`n", $utf8)

Write-Host ""
Write-Host "Median startup: $($report.startupMs.median) ms"
Write-Host "Median working set: $([math]::Round($report.workingSetBytes.median / 1MB, 1)) MiB"
Write-Host "App image: $([math]::Round($report.app.imageBytes / 1MB, 1)) MiB"
Write-Host "Report: $JsonPath"
