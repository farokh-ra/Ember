#!/usr/bin/env python3
"""Generate an Ember CycloneDX SBOM and a third-party notice bundle.

The SBOM is derived from committed lock data only:

* ``Cargo.lock`` through ``cargo metadata --locked``;
* ``gradle/verification-metadata.xml``, which records every resolved Maven
  artifact and its SHA-256 checksum.

No network access is required. Workspace crates are represented by the Ember
application component, not as third-party libraries.

The optional notice bundle copies licence and notice files from the local Cargo
registry and Gradle cache. Those files are not committed; the release pipeline
generates the bundle next to the artifacts. Missing files are reported and only
fail with ``--strict-notices``.

Usage:
    python scripts/generate_sbom.py --version 0.1.0 --sbom dist/sbom/ember.cdx.json
    python scripts/generate_sbom.py --version 0.1.0 --sbom ... --notices-dir dist/notices
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path
from urllib.parse import quote

REPO_ROOT = Path(__file__).resolve().parent.parent
VERIFICATION_METADATA = REPO_ROOT / "gradle" / "verification-metadata.xml"

# Direct Maven groups whose upstream projects publish Apache-2.0 artifacts.
APACHE_GROUPS = (
    "androidx.",
    "com.android.",
    "com.google.",
    "org.apache.",
    "org.jetbrains.",
    "org.ow2.",
)

# Components that are test-only and must not be described as runtime-required.
TEST_ONLY = (
    "junit",
    "org.junit",
    "org.opentest4j",
    "org.apiguardian",
    "org.mockito",
    "app.cash.turbine",
)

LICENSE_ENTRY = re.compile(
    r"^(?:META-INF/)?(?:LICEN[CS]E|COPYING|NOTICE)(?:[._-].*)?$",
    re.IGNORECASE,
)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_name(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9._+-]+", "-", value).strip("-") or "license"


def cargo_metadata() -> dict:
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
    except FileNotFoundError:
        raise SystemExit("cargo is not on PATH; run scripts/dev-env.ps1 first")
    except subprocess.CalledProcessError as error:
        sys.stderr.write(error.stderr)
        raise SystemExit("cargo metadata failed; the lockfile may be out of date")
    return json.loads(result.stdout)


def cargo_license(package: dict) -> str | None:
    expression = (package.get("license") or "").strip()
    if expression:
        return expression
    license_file = (package.get("license_file") or "").strip()
    if license_file:
        return f"LicenseRef-{safe_name(Path(license_file).name)}"
    return None


def cargo_component(package: dict) -> dict:
    name = package["name"]
    version = package["version"]
    purl = f"pkg:cargo/{quote(name)}@{quote(version)}"
    component = {
        "type": "library",
        "bom-ref": purl,
        "name": name,
        "version": version,
        "purl": purl,
        "scope": "required",
        "properties": [{"name": "ember:source", "value": "cargo"}],
    }
    license_expression = cargo_license(package)
    if license_expression:
        component["licenses"] = license_entry(license_expression)
    references = []
    for key, reference_type in (("repository", "vcs"), ("homepage", "website")):
        value = package.get(key)
        if value:
            references.append({"type": reference_type, "url": value})
    if references:
        component["externalReferences"] = references
    return component


def license_entry(expression: str) -> list[dict]:
    """Return a CycloneDX licence object for an SPDX expression or a name."""

    expression = expression.strip()
    if not expression:
        return []
    if expression.startswith("LicenseRef-") or re.fullmatch(
        r"[A-Za-z0-9.+\-]+(?: (?:AND|OR|WITH) [A-Za-z0-9.+\-]+)*", expression
    ):
        return [{"expression": expression}]
    return [{"license": {"name": expression}}]


def maven_license(group: str, name: str) -> str | None:
    if group == "net.java.dev.jna":
        return "Apache-2.0"
    if name.startswith("junit") or group.startswith("org.junit"):
        return "EPL-2.0"
    if group.startswith(APACHE_GROUPS):
        return "Apache-2.0"
    if group in {"org.slf4j", "org.yaml"} or group.startswith("com.fasterxml."):
        return "Apache-2.0"
    return None


def maven_components() -> tuple[list[dict], int]:
    if not VERIFICATION_METADATA.is_file():
        raise SystemExit(
            f"{VERIFICATION_METADATA.relative_to(REPO_ROOT)} is missing; run "
            "scripts/generate-verification-metadata.ps1 first"
        )
    root = ET.parse(VERIFICATION_METADATA).getroot()
    components = []
    skipped = 0
    seen = set()
    for node in root.iter():
        # The file declares the Gradle dependency-verification namespace.
        if node.tag.rsplit("}", 1)[-1] != "component":
            continue
        group = node.get("group")
        name = node.get("name")
        version = node.get("version")
        if not group or not name or not version:
            skipped += 1
            continue
        key = (group, name, version)
        if key in seen:
            continue
        seen.add(key)
        purl = f"pkg:maven/{quote(group)}/{quote(name)}@{quote(version)}"
        component = {
            "type": "library",
            "bom-ref": purl,
            "group": group,
            "name": name,
            "version": version,
            "purl": purl,
            "scope": "excluded" if (name.startswith(TEST_ONLY) or group.startswith(TEST_ONLY)) else "required",
            "externalReferences": [
                {
                    "type": "distribution",
                    "url": (
                        "https://repo1.maven.org/maven2/"
                        f"{group.replace('.', '/')}/{name}/{version}/"
                    ),
                }
            ],
            "properties": [{"name": "ember:source", "value": "maven"}],
        }
        expression = maven_license(group, name)
        if expression:
            component["licenses"] = license_entry(expression)
        if group == "net.java.dev.jna":
            component["properties"].append(
                {"name": "ember:license-selection", "value": "Apache-2.0"}
            )
        components.append(component)
    return components, skipped


def validate_sbom(sbom: dict) -> None:
    if sbom.get("bomFormat") != "CycloneDX" or sbom.get("specVersion") != "1.5":
        raise SystemExit("SBOM header is not CycloneDX 1.5")
    components = sbom.get("components") or []
    if len(components) < 10:
        raise SystemExit("SBOM has too few components; lock data may be missing")
    references = set()
    for component in components:
        for field in ("type", "name", "version", "bom-ref", "purl"):
            if not component.get(field):
                raise SystemExit(f"SBOM component is missing {field}: {component!r}")
        if component["bom-ref"] in references:
            raise SystemExit(f"duplicate bom-ref: {component['bom-ref']}")
        references.add(component["bom-ref"])
        if not component["purl"].startswith("pkg:"):
            raise SystemExit(f"invalid purl: {component['purl']}")


def build_sbom(version: str) -> tuple[dict, dict]:
    metadata = cargo_metadata()
    workspace_members = set(metadata.get("workspace_members", []))
    rust_packages = [
        package for package in metadata["packages"] if package["id"] not in workspace_members
    ]
    rust_components = [cargo_component(package) for package in rust_packages]
    maven, skipped_maven = maven_components()
    components = sorted(
        rust_components + maven,
        key=lambda component: component["bom-ref"],
    )

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": f"pkg:generic/ember@{quote(version)}",
                "name": "Ember",
                "version": version,
                "purl": f"pkg:generic/ember@{quote(version)}",
                "licenses": [{"license": {"name": "Proprietary"}}],
            },
            "properties": [
                {"name": "ember:protocol", "value": "1"},
                {"name": "ember:identify", "value": "0.1.0-stage-12"},
                {"name": "ember:storage-schema", "value": "11"},
                {"name": "ember:rust-components", "value": str(len(rust_components))},
                {"name": "ember:maven-components", "value": str(len(maven))},
                {"name": "ember:maven-components-without-version", "value": str(skipped_maven)},
                {
                    "name": "ember:sbom-scope",
                    "value": "Build graph from committed lock data, including test and build dependencies.",
                },
            ],
        },
        "components": components,
    }
    summary = {
        "rust": len(rust_components),
        "maven": len(maven),
        "maven_without_version": skipped_maven,
        "total": len(components),
        "workspace_crates": len(workspace_members),
    }
    return sbom, summary


class LicenseStore:
    """Store unique licence texts once and map components to them."""

    def __init__(self, directory: Path):
        self.directory = directory
        self.directory.mkdir(parents=True, exist_ok=True)
        self.by_hash: dict[str, str] = {}
        self.entries: list[dict] = []

    def add(self, data: bytes, source: str) -> str | None:
        if not data or len(data) > 2_000_000:
            return None
        digest = sha256_bytes(data)
        relative = self.by_hash.get(digest)
        if relative is None:
            relative = f"licenses/{digest[:16]}-{safe_name(Path(source).name)}"
            target = self.directory / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            self.by_hash[digest] = relative
        if not any(entry["path"] == relative and entry["source"] == source for entry in self.entries):
            self.entries.append({"path": relative, "source": source, "sha256": digest})
        return relative


def cargo_license_files(package: dict, store: LicenseStore, mapping: dict) -> int:
    manifest = Path(package["manifest_path"])
    directory = manifest.parent
    if not directory.is_dir():
        return 0
    found = 0
    for candidate in sorted(directory.iterdir()):
        if candidate.is_file() and LICENSE_ENTRY.match(candidate.name):
            if store.add(candidate.read_bytes(), f"{package['name']}-{package['version']}/{candidate.name}"):
                found += 1
    if found:
        mapping[f"pkg:cargo/{package['name']}@{package['version']}"] = [
            entry["path"]
            for entry in store.entries
            if entry["source"].startswith(f"{package['name']}-{package['version']}/")
        ]
    return found


def gradle_cache() -> Path | None:
    configured = os.environ.get("GRADLE_USER_HOME")
    if configured:
        path = Path(configured)
        return path if path.is_dir() else None
    candidate = Path.home() / ".gradle"
    return candidate if candidate.is_dir() else None


def read_archive_licenses(data: bytes, label: str, store: LicenseStore) -> int:
    found = 0
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for name in archive.namelist():
                normalized = name.replace("\\", "/")
                if not LICENSE_ENTRY.match(normalized.split("/")[-1]):
                    continue
                # Android AARs keep licences under META-INF/<group>/<name>/;
                # allow that depth while rejecting generated source trees.
                if normalized.count("/") > 8:
                    continue
                info = archive.getinfo(name)
                if info.file_size > 2_000_000:
                    continue
                payload = archive.read(name)
                if store.add(payload, f"{label}/{normalized}"):
                    found += 1
    except (zipfile.BadZipFile, OSError):
        return 0
    return found


def maven_license_files(component: dict, cache: Path, store: LicenseStore, mapping: dict) -> int:
    group = component["group"]
    name = component["name"]
    version = component["version"]
    base = cache / "caches" / "modules-2" / "files-2.1" / group / name / version
    if not base.is_dir():
        base = cache / "modules-2" / "files-2.1" / group / name / version
    if not base.is_dir():
        return 0
    found = 0
    for archive in sorted(base.glob("*/*")):
        if archive.suffix.lower() == ".jar":
            found += read_archive_licenses(archive.read_bytes(), f"{group}-{name}-{version}", store)
        elif archive.suffix.lower() == ".aar":
            try:
                with zipfile.ZipFile(archive) as aar:
                    # AndroidX AARs usually keep the notice at the archive root
                    # under META-INF/<group>/<name>/, not inside classes.jar.
                    found += read_archive_licenses(
                        archive.read_bytes(), f"{group}-{name}-{version}", store
                    )
                    try:
                        nested = aar.read("classes.jar")
                    except KeyError:
                        continue
                found += read_archive_licenses(
                    nested, f"{group}-{name}-{version}/classes", store
                )
            except (zipfile.BadZipFile, OSError):
                continue
    if found:
        mapping[component["bom-ref"]] = [
            entry["path"]
            for entry in store.entries
            if entry["source"].startswith(f"{group}-{name}-{version}/")
        ]
    return found


def jdk_license_files(store: LicenseStore) -> int:
    java_home = os.environ.get("JAVA_HOME")
    if not java_home:
        return 0
    legal = Path(java_home) / "legal"
    if not legal.is_dir():
        return 0
    found = 0
    for path in sorted(legal.rglob("*")):
        if path.is_file() and path.stat().st_size <= 2_000_000:
            relative = path.relative_to(legal).as_posix()
            if store.add(path.read_bytes(), f"jdk/{relative}"):
                found += 1
    return found


def write_notices(
    version: str,
    sbom: dict,
    summary: dict,
    directory: Path,
    strict: bool,
) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    store = LicenseStore(directory)
    mapping: dict[str, list[str]] = {}
    rust = [component for component in sbom["components"] if component["properties"][0]["value"] == "cargo"]
    maven = [component for component in sbom["components"] if component["properties"][0]["value"] == "maven"]

    metadata = cargo_metadata()
    packages = {f"{package['name']}@{package['version']}": package for package in metadata["packages"]}
    missing_rust = []
    for component in rust:
        package = packages.get(f"{component['name']}@{component['version']}")
        if package is None or cargo_license_files(package, store, mapping) == 0:
            missing_rust.append(component["name"])

    cache = gradle_cache()
    missing_maven = []
    if cache is not None:
        for component in maven:
            if maven_license_files(component, cache, store, mapping) == 0:
                missing_maven.append(component["bom-ref"])
    else:
        missing_maven = [component["bom-ref"] for component in maven]

    jdk_files = jdk_license_files(store)

    lines = [
        "# Ember third-party notices",
        "",
        f"**Version:** {version}",
        f"**Generated from:** `Cargo.lock` and `gradle/verification-metadata.xml`",
        "",
        "Ember itself is proprietary software. This bundle covers third-party",
        "components that are linked, compiled, or bundled with an Ember build.",
        "It is generated from locked dependency metadata and local licence files.",
        "It is not a legal opinion; see `docs/release/SBOM_AND_NOTICES.md`.",
        "",
        "## Bundled runtime",
        "",
        "| Component | Licence | Note |",
        "|---|---|---|",
        "| Eclipse Temurin JDK 21 | GPL-2.0-with-Classpath-Exception | Bundled by jpackage on Windows; license files are in `licenses/`. |",
        "| SQLite | Public domain | Compiled into the Rust core through `libsqlite3-sys`. |",
        "| Skiko and Skia | Apache-2.0 and BSD-3-Clause | Compose Desktop native runtime; see the Skiko components below. |",
        "| Kotlin, Compose, and AndroidX | Apache-2.0 | Maven components below. |",
        "",
        "## Summary",
        "",
        f"- Rust third-party components: {summary['rust']}",
        f"- Maven components: {summary['maven']}",
        f"- Workspace crates represented by the application component: {summary['workspace_crates']}",
        f"- Unique licence and notice texts: {len(store.by_hash)}",
        f"- Rust components with no licence file found: {len(missing_rust)}",
        f"- Maven components with no licence file found: {len(missing_maven)}",
        f"- Bundled JDK licence files: {jdk_files}",
        "",
        "## Rust components",
        "",
        "| Component | Version | Licence | Repository |",
        "|---|---|---|---|",
    ]
    for component in rust:
        licence = component.get("licenses", [{}])[0]
        expression = licence.get("expression") or licence.get("license", {}).get("name") or "NOASSERTION"
        repository = next(
            (
                reference["url"]
                for reference in component.get("externalReferences", [])
                if reference["type"] == "vcs"
            ),
            "",
        )
        lines.append(
            f"| {component['name']} | {component['version']} | {expression} | {repository} |"
        )

    lines.extend(
        [
            "",
            "## Maven components",
            "",
            "| Group | Component | Version | Licence |",
            "|---|---|---|---|",
        ]
    )
    for component in maven:
        licence = component.get("licenses", [{}])[0]
        expression = licence.get("expression") or licence.get("license", {}).get("name") or "NOASSERTION"
        lines.append(
            f"| {component['group']} | {component['name']} | {component['version']} | {expression} |"
        )

    lines.extend(
        [
            "",
            "## Licence texts",
            "",
            "Unique texts are stored under `licenses/`. The machine-readable mapping",
            "from component to stored text is in `license-files.json`. Components",
            "without a stored text either declare their licence in the table above or",
            "must be checked against the upstream artifact before a public release.",
            "",
        ]
    )
    (directory / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines), encoding="utf-8")
    (directory / "license-files.json").write_text(
        json.dumps(mapping, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (directory / "sbom.cdx.json").write_text(
        json.dumps(sbom, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    result = {
        "licence_texts": len(store.by_hash),
        "rust_missing": missing_rust,
        "maven_missing": missing_maven,
        "jdk_files": jdk_files,
    }
    if strict and (missing_rust or missing_maven):
        sys.stderr.write(
            "missing licence files: "
            f"{len(missing_rust)} Rust, {len(missing_maven)} Maven\n"
        )
        raise SystemExit(1)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", default="0.1.0", help="product version recorded in the SBOM")
    parser.add_argument("--sbom", type=Path, help="write the CycloneDX JSON here")
    parser.add_argument("--notices-dir", type=Path, help="write the notice bundle here")
    parser.add_argument(
        "--strict-notices",
        action="store_true",
        help="fail when a component has no local licence file",
    )
    args = parser.parse_args()
    if not args.sbom and not args.notices_dir:
        parser.error("at least one of --sbom or --notices-dir is required")

    sbom, summary = build_sbom(args.version)
    validate_sbom(sbom)
    rendered = json.dumps(sbom, indent=2, ensure_ascii=False) + "\n"
    if args.sbom:
        args.sbom.parent.mkdir(parents=True, exist_ok=True)
        args.sbom.write_text(rendered, encoding="utf-8")
        print(
            f"SBOM: {args.sbom} ({summary['total']} components; "
            f"{summary['rust']} Rust, {summary['maven']} Maven)"
        )
    if args.notices_dir:
        result = write_notices(args.version, sbom, summary, args.notices_dir, args.strict_notices)
        print(
            f"Notices: {args.notices_dir} ({result['licence_texts']} licence texts; "
            f"{len(result['rust_missing'])} Rust and {len(result['maven_missing'])} Maven "
            "components without a local licence file)"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
