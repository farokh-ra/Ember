#!/usr/bin/env python3
"""Validate the Stage 02 architecture and decision documents.

Checks:
- Required Stage 02 documents exist.
- Ten ADRs exist, are accepted, and contain the required sections.
- The ADR index links every ADR.
- The architecture defines Kotlin modules and Rust crates with responsibilities.
- The FFI draft has a unique method catalog, both adapters, and a header.
- The dependency inventory pins direct dependencies and licenses.
- Local markdown links resolve and markdown tables have consistent widths.
- No TODO/TBD/FIXME/XXX markers remain in Stage 02 documents.

Exit code 0 means all checks passed.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
ARCH = DOCS / "architecture"
DECISIONS = DOCS / "decisions"

REQUIRED = [
    DOCS / "ARCHITECTURE.md",
    DECISIONS / "README.md",
    *[DECISIONS / f"ADR-{n:04d}-{slug}.md" for n, slug in [
        (1, "kotlin-compose-multiplatform"),
        (2, "rust-core-boundary"),
        (3, "ffi-c-abi-jni-jna"),
        (4, "rust-storage-ownership"),
        (5, "libp2p-transports-discovery"),
        (6, "crypto-and-record-formats"),
        (7, "protocol-boundaries-versioning"),
        (8, "lifecycle-background-execution"),
        (9, "build-packaging-dependency-policy"),
        (10, "search-index-boundary"),
    ]],
    ARCH / "COMPONENT_DIAGRAMS.md",
    ARCH / "FFI_INTERFACE.md",
    ARCH / "nexus_core.h",
    ARCH / "DEPENDENCY_LICENSE_INVENTORY.md",
    DOCS / "progress" / "STAGE_02.md",
    DOCS / "HANDOFF.md",
]

ADR_FILES = [path for path in REQUIRED if re.search(r"ADR-\d{4}-", path.name)]
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
METHOD_RE = re.compile(r"^\|\s*(0x[0-9A-Fa-f]{4})\s*\|")
FUNC_RE = re.compile(r"\bnexus_core_[a-z][a-z0-9_]*\b")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def check_links(paths: list[Path]) -> list[str]:
    failures: list[str] = []
    for path in paths:
        for target in LINK_RE.findall(read(path)):
            target = target.strip()
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            target_path = target.split("#", 1)[0]
            if not target_path:
                continue
            resolved = (path.parent / target_path).resolve()
            if not resolved.exists():
                failures.append(f"{path.relative_to(ROOT)} -> missing link target {target_path}")
    return failures


def check_tables(paths: list[Path]) -> list[str]:
    failures: list[str] = []
    for path in paths:
        in_fence = False
        block: list[tuple[int, str]] = []

        def validate() -> None:
            if len(block) < 2:
                return
            widths = []
            for line_number, line in block:
                cells = line.strip().strip("|").replace("\\|", "|").split("|")
                widths.append((line_number, len(cells), line))
            expected = widths[0][1]
            for line_number, width, _ in widths[1:]:
                if width != expected:
                    failures.append(
                        f"{path.relative_to(ROOT)}:{line_number} has {width} table cells; "
                        f"header has {expected}"
                    )

        for line_number, line in enumerate(read(path).splitlines(), 1):
            if line.strip().startswith("```"):
                in_fence = not in_fence
                validate()
                block = []
                continue
            if in_fence:
                continue
            if line.strip().startswith("|"):
                block.append((line_number, line))
            else:
                validate()
                block = []
        validate()
    return failures


def section(text: str, start: str, end: str | None = None) -> str:
    index = text.find(start)
    if index == -1:
        return ""
    if end is None:
        return text[index:]
    end_index = text.find(end, index + len(start))
    return text[index:] if end_index == -1 else text[index:end_index]


def table_rows(block: str, prefix: str) -> list[list[str]]:
    rows: list[list[str]] = []
    for line in block.splitlines():
        stripped = line.strip()
        if stripped.startswith(prefix):
            rows.append([cell.strip() for cell in stripped.strip("|").split("|")])
    return rows


def main() -> int:
    failures: list[str] = []

    for path in REQUIRED:
        if not path.exists():
            failures.append(f"Missing required file: {path.relative_to(ROOT)}")

    if failures:
        print("STAGE 02 VALIDATION: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    architecture = read(DOCS / "ARCHITECTURE.md")
    adr_index = read(DECISIONS / "README.md")
    ffi = read(ARCH / "FFI_INTERFACE.md")
    header = read(ARCH / "nexus_core.h")
    deps = read(ARCH / "DEPENDENCY_LICENSE_INVENTORY.md")
    diagrams = read(ARCH / "COMPONENT_DIAGRAMS.md")
    progress = read(DOCS / "progress" / "STAGE_02.md")

    # ADR structure.
    accepted_adrs = 0
    for path in ADR_FILES:
        text = read(path)
        adr_id = path.name.split("-")[1]
        if "**Status:** Accepted" not in text:
            failures.append(f"{path.relative_to(ROOT)} is not marked Accepted")
        else:
            accepted_adrs += 1
        for heading in (
            "## Context",
            "## Decision",
            "## Consequences",
            "## Alternatives considered",
            "## Verification",
        ):
            if heading not in text:
                failures.append(f"{path.relative_to(ROOT)} is missing {heading}")
        link = f"]({path.name})"
        if link not in adr_index:
            failures.append(f"ADR index does not link {path.name}")
        if f"ADR-{adr_id}" not in adr_index:
            failures.append(f"ADR index does not name ADR-{adr_id}")

    # Architecture sections and responsibilities.
    for heading in (
        "## 3. System context",
        "## 4. Components and responsibilities",
        "### 4.2 Kotlin modules",
        "### 4.3 Rust crates",
        "## 6. FFI boundary",
        "## 7. Storage architecture",
        "## 8. Protocol boundaries",
        "## 12. Security architecture",
        "## 13. Lifecycle and failure handling",
        "## 14. Platform parity",
        "## 17. Fallback matrix",
        "## 18. Verification and stage mapping",
        "## 21. Terminology",
    ):
        if heading not in architecture:
            failures.append(f"ARCHITECTURE.md is missing section {heading}")

    for phrase in (
        "No central backend rule",
        "one versioned C ABI",
        "Rust owns all durable state",
        "Android and Windows use the same Rust core API",
    ):
        if phrase not in architecture:
            failures.append(f"ARCHITECTURE.md is missing required statement: {phrase}")

    kotlin_block = section(architecture, "### 4.2 Kotlin modules", "### 4.3 Rust crates")
    rust_block = section(architecture, "### 4.3 Rust crates", "### 4.4 Responsibility rules")
    kotlin_rows = table_rows(kotlin_block, "| `")
    rust_rows = table_rows(rust_block, "| `nexus-")
    if len(kotlin_rows) < 7:
        failures.append(f"Kotlin module table has {len(kotlin_rows)} rows; expected at least 7")
    if len(rust_rows) < 12:
        failures.append(f"Rust crate table has {len(rust_rows)} rows; expected at least 12")
    for row in kotlin_rows + rust_rows:
        if len(row) < 3:
            failures.append(f"Component responsibility row is too narrow: {row[0] if row else '?'}")

    # FFI catalog and header parity.
    method_ids: list[str] = []
    method_rows = 0
    for line in ffi.splitlines():
        match = METHOD_RE.match(line)
        if match:
            method_ids.append(match.group(1))
            method_rows += 1
    if method_rows < 40:
        failures.append(f"FFI method catalog has {method_rows} methods; expected at least 40")
    if len(method_ids) != len(set(method_ids)):
        failures.append("FFI method catalog contains duplicate method IDs")

    for phrase in (
        "JNI",
        "JNA",
        "Android and Windows",
        "No native-to-JVM callbacks",
        "nexus_core_next_event",
        "nexus_core_free_buffer",
        "NEXUS_STATUS_ABI_MISMATCH",
    ):
        if phrase not in ffi:
            failures.append(f"FFI_INTERFACE.md is missing required statement: {phrase}")

    doc_functions = {name for name in FUNC_RE.findall(ffi) if not name.endswith("_")}
    missing_header_functions = sorted(name for name in doc_functions if name not in header)
    if missing_header_functions:
        failures.append(f"Header is missing FFI functions: {', '.join(missing_header_functions)}")
    for symbol in ("NEXUS_ABI_VERSION", "NexusStatus", "NexusBuffer", "nexus_core_import_begin", "nexus_core_export_read"):
        if symbol not in header:
            failures.append(f"Header is missing {symbol}")

    # Dependency inventory.
    for heading in (
        "## 2. License policy summary",
        "## 3. Toolchain pins",
        "## 4. Rust direct dependencies",
        "## 5. Kotlin, Compose, and JVM dependencies",
        "## 7. Transitive and platform risks",
        "## 9. Verification and update process",
    ):
        if heading not in deps:
            failures.append(f"DEPENDENCY_LICENSE_INVENTORY.md is missing {heading}")

    rust_deps_block = section(deps, "## 4. Rust direct dependencies", "## 5. Kotlin")
    kotlin_deps_block = section(deps, "## 5. Kotlin", "## 6. Build and packaging")
    rust_deps = table_rows(rust_deps_block, "| ")
    kotlin_deps = table_rows(kotlin_deps_block, "| ")
    rust_deps = [row for row in rust_deps if len(row) >= 6 and row[0] not in ("Dependency", "---")]
    kotlin_deps = [row for row in kotlin_deps if len(row) >= 6 and row[0] not in ("Dependency", "---")]
    if len(rust_deps) < 20:
        failures.append(f"Rust dependency table has {len(rust_deps)} rows; expected at least 20")
    if len(kotlin_deps) < 10:
        failures.append(f"Kotlin dependency table has {len(kotlin_deps)} rows; expected at least 10")

    for label, rows in (("Rust", rust_deps), ("Kotlin", kotlin_deps)):
        for row in rows:
            license_cell = row[2]
            if not license_cell:
                failures.append(f"{label} dependency {row[0]} has no license")
            if re.search(r"(?<!L)GPL|AGPL|SSPL", license_cell):
                failures.append(f"{label} dependency {row[0]} has disallowed license {license_cell}")
            if any(marker in license_cell for marker in ("TBD", "TODO", "FIXME", "unknown")):
                failures.append(f"{label} dependency {row[0]} has unresolved license {license_cell}")

    # Toolchain pins must agree between the architecture and the inventory.
    # The inventory stores the tool name and version in separate table cells,
    # so compare the version tokens rather than a single combined string.
    for tool, pin in (
        ("Kotlin", "2.4.21"),
        ("Compose Multiplatform", "1.12.2"),
        ("Android Gradle Plugin", "9.5.1"),
        ("Rust", "1.99.1"),
        ("Gradle", "9.9.1"),
        ("Android NDK", "29.0.14206865"),
    ):
        if pin not in architecture:
            failures.append(f"ARCHITECTURE.md does not state toolchain pin {tool} {pin}")
        if pin not in deps:
            failures.append(f"Dependency inventory does not state toolchain pin {tool} {pin}")

    # Diagram and stage records.
    if diagrams.count("```mermaid") < 10:
        failures.append(f"COMPONENT_DIAGRAMS.md has {diagrams.count('```mermaid')} Mermaid diagrams; expected at least 10")
    if "Stage 02" not in progress or "Stage 03" not in progress:
        failures.append("STAGE_02.md must describe Stage 02 and the next Stage 03")
    if "**Status:** Complete" not in progress:
        failures.append("STAGE_02.md does not record Stage 02 as complete")

    # No unresolved markers in Stage 02 documents.
    stage02_docs = [
        DOCS / "ARCHITECTURE.md",
        *ADR_FILES,
        ARCH / "COMPONENT_DIAGRAMS.md",
        ARCH / "FFI_INTERFACE.md",
        ARCH / "DEPENDENCY_LICENSE_INVENTORY.md",
        DOCS / "progress" / "STAGE_02.md",
        DOCS / "HANDOFF.md",
    ]
    for path in stage02_docs:
        text = read(path)
        for marker in ("TODO", "TBD", "FIXME", "XXX"):
            if marker in text:
                failures.append(f"{path.relative_to(ROOT)} contains unresolved marker {marker}")

    link_failures = check_links(sorted(DOCS.rglob("*.md")))
    failures.extend(link_failures)
    table_failures = check_tables(sorted(DOCS.rglob("*.md")))
    failures.extend(table_failures)

    print("STAGE 02 ARCHITECTURE VALIDATION")
    print(f"  Required documents: {len(REQUIRED)}/{len(REQUIRED)} present")
    print(f"  ADRs checked: {len(ADR_FILES)} (accepted: {accepted_adrs})")
    print(f"  Kotlin modules: {len(kotlin_rows)}")
    print(f"  Rust components: {len(rust_rows)}")
    print(f"  FFI methods: {method_rows}")
    print(f"  FFI C functions in header: {len(set(FUNC_RE.findall(header)))}")
    print(f"  Rust dependencies inventoried: {len(rust_deps)}")
    print(f"  Kotlin and JVM dependencies inventoried: {len(kotlin_deps)}")
    print(f"  Local markdown links checked: {'yes' if not link_failures else 'failures found'}")
    print(f"  Markdown tables checked: {'yes' if not table_failures else 'failures found'}")

    if failures:
        print("\nSTAGE 02 VALIDATION: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("\nSTAGE 02 VALIDATION: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
