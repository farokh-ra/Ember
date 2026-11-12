#!/usr/bin/env python3
"""Validate the Stage 01 product documents.

Checks:
- Required documents exist.
- Acceptance-criterion IDs are defined exactly once.
- Every referenced criterion ID (including ranges) resolves.
- Every P0 feature row in the priority matrix has at least one criterion.
- Every referenced link in docs/ resolves to an existing local file.
- No TODO/TBD/FIXME markers remain in approved documents.

Exit code 0 means all checks passed.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"

REQUIRED = [
    DOCS / "PRODUCT_REQUIREMENTS.md",
    DOCS / "product" / "GLOSSARY.md",
    DOCS / "product" / "USER_FLOWS.md",
    DOCS / "product" / "FEATURE_PRIORITY_MATRIX.md",
    DOCS / "product" / "ACCEPTANCE_CRITERIA.md",
    DOCS / "progress" / "STAGE_01.md",
    DOCS / "HANDOFF.md",
]

PREFIXES = (
    "PLAT|ONB|ID|PROF|DATA|HASH|PUB|NET|BOOT|DISC|SEARCH|DL|REP|UX|A11Y|L10N|SEC|NFR|REL"
)
ID_RE = re.compile(rf"\b(?:{PREFIXES})-\d{{3}}\b")
DEF_RE = re.compile(rf"^\*\*((?:{PREFIXES})-\d{{3}})\b")
RANGE_RE = re.compile(
    rf"\b(?P<prefix>{PREFIXES})-(?P<start>\d{{3}})\s*[–-]\s*(?:(?P=prefix)-)?(?P<end>\d{{3}})\b"
)
FR_RE = re.compile(r"\bFR-[A-Z]+-\d{2}\b")
FR_RANGE_RE = re.compile(
    r"\bFR-(?P<prefix>[A-Z]+)-(?P<start>\d{2})\s*[–-]\s*(?:FR-(?P=prefix)-)?(?P<end>\d{2})\b"
)
FR_DEF_RE = re.compile(r"^\|\s*(FR-[A-Z]+-\d{2})\s*\|")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def expand_ranges(text: str) -> set[str]:
    ids = set(ID_RE.findall(text))
    for match in RANGE_RE.finditer(text):
        prefix = match.group("prefix")
        start = int(match.group("start"))
        end = int(match.group("end"))
        if end < start:
            start, end = end, start
        for number in range(start, end + 1):
            ids.add(f"{prefix}-{number:03d}")
    return ids


def expand_fr(text: str) -> set[str]:
    ids = set(FR_RE.findall(text))
    for match in FR_RANGE_RE.finditer(text):
        prefix = match.group("prefix")
        start = int(match.group("start"))
        end = int(match.group("end"))
        if end < start:
            start, end = end, start
        for number in range(start, end + 1):
            ids.add(f"FR-{prefix}-{number:02d}")
    return ids


def parse_fr_defined(text: str) -> tuple[dict[str, int], list[str]]:
    counts: dict[str, int] = {}
    order: list[str] = []
    for line in text.splitlines():
        match = FR_DEF_RE.match(line)
        if match:
            fr_id = match.group(1)
            counts[fr_id] = counts.get(fr_id, 0) + 1
            order.append(fr_id)
    return counts, order


def parse_defined(text: str) -> tuple[dict[str, int], list[str]]:
    counts: dict[str, int] = {}
    order: list[str] = []
    for line in text.splitlines():
        match = DEF_RE.match(line)
        if match:
            ac_id = match.group(1)
            counts[ac_id] = counts.get(ac_id, 0) + 1
            order.append(ac_id)
    return counts, order


def parse_p0_rows(text: str) -> list[list[str]]:
    start = text.find("## P0 —")
    end = text.find("## P1 —")
    if start == -1 or end == -1 or end <= start:
        raise ValueError("Could not locate P0 table boundaries")
    rows: list[list[str]] = []
    for line in text[start:end].splitlines():
        stripped = line.strip()
        if not stripped.startswith("| F-"):
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if len(cells) < 5:
            raise ValueError(f"Malformed P0 matrix row: {stripped}")
        rows.append(cells)
    return rows


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


def check_links(paths: list[Path]) -> list[str]:
    failures: list[str] = []
    for path in paths:
        text = read(path)
        for target in LINK_RE.findall(text):
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


def main() -> int:
    failures: list[str] = []

    for path in REQUIRED:
        if not path.exists():
            failures.append(f"Missing required file: {path.relative_to(ROOT)}")

    if failures:
        print("STAGE 01 VALIDATION: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    prd = read(DOCS / "PRODUCT_REQUIREMENTS.md")
    matrix = read(DOCS / "product" / "FEATURE_PRIORITY_MATRIX.md")
    acceptance = read(DOCS / "product" / "ACCEPTANCE_CRITERIA.md")
    flows = read(DOCS / "product" / "USER_FLOWS.md")
    glossary = read(DOCS / "product" / "GLOSSARY.md")

    counts, order = parse_defined(acceptance)
    duplicates = sorted(ac_id for ac_id, count in counts.items() if count > 1)
    if duplicates:
        failures.append(f"Duplicate acceptance criteria definitions: {', '.join(duplicates)}")

    defined = set(counts)
    referenced = expand_ranges(prd) | expand_ranges(matrix) | expand_ranges(flows) | expand_ranges(glossary)
    missing_defs = sorted(referenced - defined)
    if missing_defs:
        failures.append(f"Referenced criteria with no definition: {', '.join(missing_defs)}")

    fr_counts, fr_order = parse_fr_defined(prd)
    fr_duplicates = sorted(fr_id for fr_id, count in fr_counts.items() if count > 1)
    if fr_duplicates:
        failures.append(f"Duplicate requirement definitions: {', '.join(fr_duplicates)}")
    fr_defined = set(fr_counts)
    fr_referenced = expand_fr(matrix) | expand_fr(flows) | expand_fr(glossary)
    missing_fr = sorted(fr_referenced - fr_defined)
    if missing_fr:
        failures.append(f"Referenced requirements with no definition: {', '.join(missing_fr)}")

    fr_without_ac: list[str] = []
    for line in prd.splitlines():
        if not FR_DEF_RE.match(line):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 3 or not expand_ranges(cells[-1]):
            fr_without_ac.append(cells[0])
    if fr_without_ac:
        failures.append(f"Requirements without acceptance criteria: {', '.join(fr_without_ac)}")

    referenced_by_product = expand_ranges(prd) | expand_ranges(matrix)
    orphaned = sorted(defined - referenced_by_product)
    if orphaned:
        failures.append(f"Defined criteria not referenced by PRD/matrix: {', '.join(orphaned)}")

    try:
        p0_rows = parse_p0_rows(matrix)
    except ValueError as exc:
        failures.append(str(exc))
        p0_rows = []

    p0_without_ac: list[str] = []
    for cells in p0_rows:
        feature_id = cells[0]
        acceptance_cell = cells[-1]
        if not expand_ranges(acceptance_cell):
            p0_without_ac.append(feature_id)
    if p0_without_ac:
        failures.append(f"P0 rows without acceptance criteria: {', '.join(p0_without_ac)}")

    link_failures = check_links(sorted(DOCS.rglob("*.md")))
    failures.extend(link_failures)

    table_failures = check_tables(sorted(DOCS.rglob("*.md")))
    failures.extend(table_failures)

    approved_text = "\n".join(
        read(path) for path in REQUIRED if path.suffix == ".md"
    )
    markers = [
        (path, marker)
        for path in REQUIRED
        for marker in ("TODO", "TBD", "FIXME")
        if marker in read(path)
    ]
    if markers:
        for path, marker in markers:
            failures.append(f"Approved document contains {marker}: {path.relative_to(ROOT)}")

    print("STAGE 01 DOCUMENTATION VALIDATION")
    print(f"  Required documents: {len(REQUIRED)}/{len(REQUIRED)} present")
    print(f"  Acceptance criteria defined: {len(defined)} (unique: {len(defined) == len(order)})")
    print(f"  Acceptance criteria referenced by PRD/matrix: {len(referenced_by_product)}")
    print(f"  Functional requirements defined: {len(fr_defined)} (unique: {len(fr_defined) == len(fr_order)})")
    print(f"  Functional requirements referenced by matrix/flows: {len(fr_referenced)}")
    print(f"  P0 feature rows checked: {len(p0_rows)}")
    print(f"  Local markdown links checked: {'yes' if not link_failures else 'failures found'}")
    print(f"  Markdown tables checked: {'yes' if not table_failures else 'failures found'}")

    if failures:
        print("\nSTAGE 01 VALIDATION: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("\nSTAGE 01 VALIDATION: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
