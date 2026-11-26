# Performance report - Ember 0.1.0

**Measured:** 2026-11-26
**Platform:** Windows 10 Pro 22H2 (build 19045), x64
**Artifact:** jpackage application image and per-user MSI built from this
repository at the Stage 19 commit
**Script:** `scripts/measure-performance.ps1`

This report records what was measured on the available machine. Android runtime,
battery, and network measurements were not possible because no emulator, device,
or shaped network was available. Those gaps are listed explicitly rather than
estimated.

## 1. Machine

| Item | Value |
|---|---|
| OS | Microsoft Windows 10 Pro, 10.0.19045 |
| CPU | Intel Core i5-10300H @ 2.50 GHz |
| Logical processors | 8 |
| Physical memory | 16 GB (17,008,955,392 bytes) |
| Storage | Local SSD (build and application directories) |

## 2. Method

`scripts/measure-performance.ps1` starts the packaged `Ember.exe`, waits for the
Java child process to report a main window, records the elapsed time, waits five
seconds for memory to settle, samples the working set, and then leaves the window
idle for the soak period while sampling total process CPU. Three runs were
performed. The median is reported with the full sample set.

Startup therefore measures launcher start to first window, not a warm JVM start
and not a test harness.

## 3. Results

| Metric | Result |
|---|---|
| Startup, median of 3 | **2,805 ms** |
| Startup samples | 2,805 / 2,938 / 2,719 ms |
| Working set after 5 s, median | **207.1 MiB** (217,124,864 bytes) |
| Working set samples | 207.1 / 200.5 / 207.5 MiB |
| Idle CPU over 180 s | **0.021% of the machine**, 0.165% of one core |
| Working-set growth over 180 s idle | **0.9 MiB** (925,696 bytes) |
| Desktop application image | **154.3 MiB** (161,810,737 bytes) |
| Windows MSI | **71.0 MiB** (74,465,280 bytes) |
| Windows portable ZIP | **69.7 MiB** (73,063,852 bytes) |
| Android APK (arm64-v8a + x86_64) | **61.3 MiB** (64,282,841 bytes) |
| Android AAB | **22.2 MiB** (23,285,459 bytes) |

The installed application started in **2,887 ms** on the same machine, consistent
with the image measurement. The portable ZIP started in **2,890 ms**.

## 4. Interpretation

- The first window appears in under three seconds on this machine. Most of the
  time is JVM and Compose/Skiko initialisation; the UI itself is a demo shell and
  does no network or database work at startup.
- Idle CPU is effectively zero. A hidden or unfocused window does not spin, and
  the 180-second soak grew the working set by less than 1 MiB, which is within
  normal JVM heap noise for an idle window.
- Artifact size is dominated by the bundled JDK and Compose/Skiko runtime. The
  Windows image also carries the native core (`nexus_core.dll`, about 30 MiB
  locally) under `app/resources`, and the APK carries the Rust core for both
  packaged ABIs. Android release libraries are stripped of debug sections during
  packaging.
- The Windows MSI is slightly larger than the portable ZIP because the MSI
  cabinet and metadata compress the same app image a little less efficiently
  than a ZIP.

## 5. Targets

No numeric performance target was agreed before Stage 19. The following
conservative targets are proposed for the first alpha and are marked against
this evidence:

| Target | Proposed | Result |
|---|---|---|
| Windows cold startup to first window | under 3,000 ms on the baseline x64 machine | Met (2,805 ms median) |
| Windows idle CPU | under 0.5% of the machine | Met (0.021%) |
| Windows 3-minute idle memory growth | under 5 MiB | Met (0.9 MiB) |
| Windows idle working set | under 350 MiB | Met (207 MiB) |
| Android cold startup | under 3,000 ms on a 4 GB device | **Not measured** |
| Android idle memory | under 250 MiB | **Not measured** |
| Battery drain | under 2%/h at idle | **Not measured** |

The Windows targets are met. Android targets remain unverified and must be
measured on real low-memory and modern devices before a public release.

## 6. Not measured

| Area | Reason |
|---|---|
| Android startup, RAM, CPU, and battery | No emulator or device in this environment. |
| Low-memory Android behaviour | No 2-4 GB device available. |
| Poor Wi-Fi, mobile data, IPv4/IPv6, network changes | No shaped network, mobile radio, or device. |
| Long-running transfer throughput and reliability | No live multi-peer network; earlier stages measured scheduler simulations and loopback transfers only. |
| Sleep/wake cycles | Suspending the shared machine is unsafe for this environment. |
| Disk I/O under concurrent transfers | UI is not yet wired to the core, so a realistic workload cannot be driven from the packaged app. |
| Network overhead | No runtime networking in the packaged UI. |

These are release-evidence gaps, not passes. They are repeated in
[COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md).

## 7. Optimisation work in this stage

- Android release libraries are stripped of debug information during
  `scripts/build-native.ps1` packaging; exported symbols are re-verified after
  stripping.
- The native Windows core is staged through `appResourcesRootDir` so it is
  packaged into the app image and the MSI rather than copied after packaging.
- The desktop package uses a minimal jlink runtime selected by Compose Desktop;
  `includeAllModules` is not enabled.
- No chunk-size, hashing, or database-index change was made. Those paths are not
  exercised by the packaged demo UI, and changing them without a realistic
  workload would be unverified churn. A benchmark harness belongs with the UI
  integration work.
- Android minification is disabled. R8 shrinking was not introduced because it
  cannot be regression-tested without a device or emulator.

## 8. Reproduce

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
. .\scripts\dev-env.ps1
.\scripts\build-native.ps1 -Target Windows -Configuration Release
.\gradlew.bat :apps:desktopApp:createDistributable
.\scripts\measure-performance.ps1 -Runs 3 -SoakSeconds 180
```

Close any running Ember instance first. The script refuses to terminate an
application it did not start.
