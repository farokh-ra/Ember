# Signing, checksums, and reproducible builds

**Applies to:** Ember 0.1.0 release artifacts

## 1. Artifacts

`scripts/package-release.ps1` produces one directory per version:

| File | Kind | Signature |
|---|---|---|
| `Ember-<version>-android.apk` | Android package | APK Signature Scheme v2 (and JAR signing when enabled by the signer) |
| `Ember-<version>-android.aab` | Android App Bundle | JAR signature |
| `Ember-<version>-windows.msi` | Per-user Windows installer | Authenticode |
| `Ember-<version>-windows-portable.zip` | Portable application image | No container signature; `Ember.exe` inside is Authenticode signed |
| `SHA256SUMS` | Checksums | Plain text |
| `release-manifest.json` | Build metadata and per-artifact digests | Plain JSON |

`verify-release.ps1` re-hashes every artifact, compares the manifest with
`SHA256SUMS`, verifies the APK and AAB signatures, checks the MSI signer is
present, and confirms that the APK and portable ZIP contain the native core.

## 2. Android signing

Release signing reads Gradle properties, environment variables, or an untracked
`signing.properties` file at the repository root. The environment variables are:

| Variable | Meaning |
|---|---|
| `EMBER_ANDROID_KEYSTORE` | Absolute path to the JKS/PKCS12 keystore |
| `EMBER_ANDROID_STORE_PASSWORD` | Keystore password |
| `EMBER_ANDROID_KEY_ALIAS` | Key alias |
| `EMBER_ANDROID_KEY_PASSWORD` | Key password |

The matching Gradle properties are `emberKeystore`, `emberStorePassword`,
`emberKeyAlias`, and `emberKeyPassword`. A Gradle property takes precedence over
the environment, which takes precedence over the file.

If they are absent, `assembleRelease` still produces an unsigned APK for local
inspection. `package-release.ps1` refuses to build a release without them unless
`-AllowUnsigned` is passed for a dry run.

The debug key is never a release key. The keystore, passwords, and
`signing.properties` are gitignored and must not be attached to a release.

## 3. Windows signing

The script accepts either:

- `EMBER_WINDOWS_PFX` and `EMBER_WINDOWS_PFX_PASSWORD` with `signtool.exe` on
  `PATH`, or
- `EMBER_WINDOWS_CERT_THUMBPRINT` pointing at a code-signing certificate already
  imported into `Cert:\CurrentUser\My` or `Cert:\LocalMachine\My`.

`EMBER_TIMESTAMP_URL` is optional. When set, the signature is timestamped; the
release workflow defaults to `http://timestamp.digicert.com`.

The MSI is signed after jpackage produces it. The executable inside the app image
is signed before the portable ZIP is created, so the ZIP payload is signed even
though the ZIP container is not.

## 4. Test-signing evidence

Production keys were not available. Stage 19 used an ephemeral 4096-bit RSA
Android key and a self-signed Windows code-signing certificate valid for two
days. Both were deleted after the test run and were never committed.

What the test proves:

- The Android build consumes an external keystore and produces a v2-signed APK
  and a signed AAB.
- The Windows build signs the executable and MSI, and the signer certificate is
  present on the artifacts.
- Verification detects a missing or invalid signature.

What it does not prove:

- Public trust. The self-signed certificate has status `UnknownError` because its
  chain is not trusted. The Android certificate is self-signed.
- Timestamping against a production timestamp authority.
- Key custody, hardware-token use, or certificate revocation procedures.

A production release must be signed with organization-owned certificates in the
release environment. See [SECURITY_RELEASE_CHECKLIST.md](../security/SECURITY_RELEASE_CHECKLIST.md).

## 5. Verification commands

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
. .\scripts\dev-env.ps1
.\scripts\verify-release.ps1 -Directory .\dist\0.1.0 -RequireSigned
```

Individual checks:

```powershell
# APK
& "$env:ANDROID_HOME\build-tools\36.0.0\apksigner.bat" verify --verbose --print-certs `
    .\dist\0.1.0\Ember-0.1.0-android.apk

# AAB
& "$env:JAVA_HOME\bin\jarsigner.exe" -verify .\dist\0.1.0\Ember-0.1.0-android.aab

# MSI
Get-AuthenticodeSignature .\dist\0.1.0\Ember-0.1.0-windows.msi

# Checksums
$manifest = Get-Content .\dist\0.1.0\release-manifest.json -Raw | ConvertFrom-Json
$manifest.artifacts | ForEach-Object {
    $path = Join-Path .\dist\0.1.0 $_.name
    "$($_.sha256)  $((Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.name)"
}
```

`jarsigner` exits non-zero for a self-signed certificate even when the JAR is
verified. Read its `jar verified` line and the warnings; the verification script
does exactly that and records the trust gap instead of hiding it.

## 6. WiX

jpackage needs WiX 3 to produce an MSI. `scripts/fetch-wix.ps1` downloads the
pinned archive and refuses a hash mismatch:

| Item | Value |
|---|---|
| Version | 3.14.1.8722 |
| Archive | `wix314-binaries.zip` |
| SHA-256 | `6AC824E1642D6F7277D0ED7EA09411A508F6116BA6FAE0AA5F2C7DAA2FF43D31` |
| Location | `NeededToolsHere/wix314` (gitignored) |

WiX is a build-only dependency and is not distributed.

## 7. Reproducibility

What is fixed:

- Rust toolchain, Gradle wrapper, JDK, Android SDK/build tools/NDK, and WiX are
  pinned.
- Release Rust builds use `cargo build --locked`; `Cargo.lock` is committed.
- `release-manifest.json` records the source commit, branch, toolchain versions,
  build time, platform, artifact sizes, and SHA-256 digests.
- The artifact set and checksums are generated by one script, so the manifest
  always describes the bytes that were produced.

Known limits:

- Gradle dependency locking and verification metadata are **not yet enabled**.
  Resolution is pinned by `libs.versions.toml`, the wrapper, and the repository
  configuration, but it is not byte-for-byte locked. This blocks a claim of fully
  reproducible release builds and is recorded as a pre-release gap.
- jpackage output can vary with the build JDK patch, locale, and runner image.
  Builds are reproducible in inputs and content policy, not guaranteed
  bit-identical.
- No CycloneDX SBOM is generated by the current script. The dependency inventory
  and `cargo-deny` output are the current dependency evidence.
- Windows builds on different hosts may embed different timestamps.

Until Gradle locking and SBOM generation are added, release artifacts are
**verifiable** (checksums and signatures) but not claimed **reproducible** in the
strict sense.

## 8. Release workflow

[`.github/workflows/release.yml`](../../.github/workflows/release.yml) runs on a
`v*` tag or manual dispatch. It:

1. Installs JDK 21, Android SDK packages, the NDK, Rust with Android targets, and
   Gradle.
2. Restores the Android keystore and Windows PFX from repository secrets.
3. Downloads WiX with hash verification.
4. Runs `package-release.ps1`, which builds, signs, checksums, manifests, and
   verifies.
5. Uploads the release directory and, for a tag, creates a GitHub release.

Required secrets:

| Secret | Required |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | Yes |
| `ANDROID_STORE_PASSWORD` | Yes |
| `ANDROID_KEY_ALIAS` | Yes |
| `ANDROID_KEY_PASSWORD` | Yes |
| `WINDOWS_PFX_BASE64` | Yes |
| `WINDOWS_PFX_PASSWORD` | Yes |

Optional repository variable: `EMBER_TIMESTAMP_URL`.

The workflow fails closed when a signing secret is absent.
