# Update and rollback strategy

**Applies to:** Windows MSI and Android APK/AAB releases

Ember 0.1.0 does not include an auto-update service. Updates are user-initiated
downloads from a release page. This keeps the product free of a mandatory
background service and keeps update checks out of the privacy surface.

## 1. Windows

### Version and identity

- The MSI is a per-user package. It installs under `%LOCALAPPDATA%\Ember` and
  does not require elevation.
- The upgrade UUID is fixed for the product:
  `c7c225ee-ae2c-4c58-822d-ea7bda7ac48c`. Changing it breaks upgrades and creates
  a second installed copy.
- The MSI version is `MAJOR.MINOR.PATCH`, with major and minor at most 255 and
  patch at most 65535. The shared `emberVersion` Gradle property drives the
  package version.
- Windows Installer allows an upgrade when the new package has the same upgrade
  UUID and a higher version. Installing an older MSI over a newer one is
  rejected by default.

### Upgrade procedure

1. Download the new MSI and its `SHA256SUMS`.
2. Verify the checksum.
3. Close Ember.
4. Run the MSI. A normal double-click starts the per-user install.
5. Confirm the uninstall entry shows the new version.

Stage 19 verified 0.1.0 -> 0.1.1: the newer MSI replaced the older product, the
install directory was updated, and the uninstall entry showed 0.1.1.

### Rollback

Windows Installer does not downgrade in place. To roll back:

1. Uninstall the newer version.
2. Install the previous MSI.
3. Restore user data from a backup if the newer version migrated it.

There is no automatic data rollback. Future schema migrations must either be
backward compatible or ship an explicit downgrade guard; the database layer
already refuses to open a newer schema rather than corrupting it.

### Uninstall and user data

The current packaged UI stores no core database or file-store data. The MSI
removes its install directory on uninstall. Theme preference is stored through
Java preferences in the user registry and is not removed.

This is a known gap for later stages: when the UI is wired to the Rust core, user
data must live outside the install directory, and uninstall must ask whether to
keep or remove it. ADR-0009 records that requirement. The MSI does not provide
that prompt today, so a future release must add either a custom action or a
separate data directory with an explicit keep/remove step before claiming
data-preserving uninstall.

### Portable package

The portable ZIP has no installer and no update mechanism. To update, exit Ember,
replace the extracted directory, and keep any future user-data directory
separate. To roll back, replace it with the previous ZIP.

## 2. Android

### Versioning

- `versionName` is the release version (for example `0.1.0`).
- `versionCode` is derived as `major * 10000 + minor * 100 + patch`, so
  `0.1.0` is `100`. `-PemberVersionCode` can override it for a release that needs
  a distinct code.
- Android only installs an update whose `versionCode` is greater than or equal to
  the installed one. A downgrade requires uninstalling the app, which deletes app
  data unless the platform backup rules preserved it.

### Upgrade procedure

1. Download the APK and checksum.
2. Verify the checksum.
3. Install the APK over the existing app with the same signing key.
4. A different signing key is rejected by Android; this is the intended
   protection against a substituted update.

The AAB is for store delivery. The store enforces the same signing identity and
monotonic `versionCode`.

### Rollback

Rolling back requires uninstalling the newer app and installing the older APK,
which clears local app data. There is no in-place downgrade. Back up the identity
through the app's explicit encrypted backup before a rollback. This path was not
exercised on a device in Stage 19.

## 3. Protocol and data compatibility

| Item | Rule |
|---|---|
| Protocol | `0.1.0-stage-18`; a future incompatible change requires a new version and a migration note. |
| Database schema | Version 11. Opening a newer schema fails rather than guessing. |
| Archive import | Older archives restore; new required tables would break that promise and need a migration. |
| ABI | C ABI 1. Additive methods are allowed within the ABI. |

## 4. Release channel policy

- `main` produces release candidates after stage approval.
- A release is tagged `v<version>` only after the signed artifacts are verified.
- Hotfix releases follow the same signing, checksum, and verification path; they
  do not skip checks because they are small.
- Artifacts are never rebuilt silently under the same version. A rebuild gets a
  new version and new checksums.
