# Third-Party Notices

Ember itself is proprietary software. See [LICENSE.md](LICENSE.md).

This file is the committed summary that ships inside the Windows application and
next to every release. The release pipeline also generates a complete bundle at
`Ember-<version>-third-party-notices.zip` containing the licence texts collected
from the exact locked dependencies, plus `Ember-<version>-sbom.cdx.json`.

## How the complete bundle is produced

```powershell
python .\scripts\generate_sbom.py `
  --version 0.1.0 `
  --sbom .\dist\0.1.0\Ember-0.1.0-sbom.cdx.json `
  --notices-dir .\dist\0.1.0\notices
```

The generator reads `Cargo.lock` through `cargo metadata --locked` and
`gradle/verification-metadata.xml` for the Maven graph. It copies unique licence
and notice files from the local Cargo registry and Gradle cache. The process is
documented in [docs/release/SBOM_AND_NOTICES.md](docs/release/SBOM_AND_NOTICES.md).

## Bundled runtime components

| Component | Licence | Obligation |
|---|---|---|
| Eclipse Temurin JDK 21 | GPL-2.0-with-Classpath-Exception | Bundle the licence, notices, and source-availability information. The full JDK `legal/` tree is included in the generated notice bundle. |
| SQLite | Public domain | Attribution only; compiled into the Rust core through `libsqlite3-sys`. |
| Skiko and Skia | Apache-2.0 and BSD-3-Clause | Include the Apache licence, BSD copyright notice, and notices. |
| Kotlin standard library, Compose Multiplatform, Compose Material3, kotlinx-coroutines | Apache-2.0 | Include the licence and notices. |
| AndroidX and Jetpack libraries | Apache-2.0 | Include the licence and notices. |
| JNA | Apache-2.0 (selected option) | The LGPL option is not selected; the Apache-2.0 licence applies. |
| Rust crates | Per-crate licences in the generated bundle | Include copyright and licence text; `cargo-deny` enforces the allowed set. |

Unicode data used by `unicode-normalization` is covered by the Unicode-3.0
licence. Test-only JUnit and Turbine code is not distributed in release
artifacts.

## Scope and limits

- The generated bundle is evidence, not a legal opinion.
- The SBOM describes the build graph from committed lock data, including build
  and test dependencies. It is not only the runtime closure.
- Components whose archive contains no licence file are still listed with their
  declared licence, and the generator reports the count. A public release
  requires a reviewed bundle with no unexplained component.
- The generated files are not committed because they are large and depend on the
  local toolchain and artifact cache. Their SHA-256 digests are recorded in
  `release-manifest.json`.

## Reporting a licensing problem

Open a repository issue without confidential material, or contact the project
owner through the private channel listed on the repository profile. Do not
include third-party source code or licence text you are not allowed to share.
