#!/usr/bin/env python3
"""Repository hygiene checks for Ember.

This script is intentionally dependency-free so it runs on a clean checkout
with only Python installed. It fails when:

* a forbidden file type or path is tracked or present in the repository;
* a plain-text file contains a likely credential or private key;
* a required ignore rule is missing from ``.gitignore``.

The CI workflow invokes this before any build so a leaked secret fails the
run even when the build itself would succeed.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

FORBIDDEN_NAMES = {
    ".env",
    "local.properties",
    "signing.properties",
    "keystore.properties",
    "secrets.properties",
    "id_rsa",
    "id_ed25519",
}

FORBIDDEN_SUFFIXES = {
    ".jks",
    ".keystore",
    ".p12",
    ".pfx",
    ".pem",
    ".key",
    ".apk",
    ".aab",
    ".msi",
    ".exe",
}

FORBIDDEN_DIRECTORIES = {
    ".gradle",
    ".idea",
    ".kotlin",
    "build",
    "target",
    "secrets",
    "node_modules",
}

SKIP_DIRECTORIES = {
    ".git",
    ".gradle",
    ".idea",
    ".kotlin",
    "build",
    "target",
    "node_modules",
    "dist",
    "out",
}

TEXT_SUFFIXES = {
    "",
    ".bat",
    ".cfg",
    ".cmd",
    ".conf",
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".gitmessage",
    ".gradle",
    ".h",
    ".ini",
    ".json",
    ".kts",
    ".kt",
    ".lock",
    ".md",
    ".properties",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".toml",
    ".txt",
    ".xml",
    ".yml",
    ".yaml",
}

REQUIRED_IGNORE_ENTRIES = [
    "local.properties",
    "*.jks",
    "*.keystore",
    ".env",
    "secrets/",
]

SECRET_PATTERNS = [
    (
        "private key block",
        re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
    ),
    (
        "AWS access key",
        re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    ),
    (
        "GitHub token",
        re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b"),
    ),
    (
        "Google API key",
        re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ),
    (
        "Slack token",
        re.compile(r"\bxox[abposr]-[0-9A-Za-z-]{10,}\b"),
    ),
    (
        "Stripe secret key",
        re.compile(r"\bsk_live_[0-9A-Za-z]{20,}\b"),
    ),
    (
        "credential assignment",
        re.compile(
            r"(?i)\b(api[_-]?key|secret|password|passphrase|access[_-]?token)\b"
            r"\s*[:=]\s*[\"'][A-Za-z0-9_\-./+=]{20,}[\"']"
        ),
    ),
]


def git(*arguments: str) -> list[str]:
    """Runs a read-only git command, returning an empty list when unavailable."""
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=REPO_ROOT,
            capture_output=True,
            check=False,
            text=True,
        )
    except OSError:
        return []
    if result.returncode != 0:
        return []
    return [line for line in result.stdout.splitlines() if line.strip()]


def repository_files() -> list[Path]:
    """Returns tracked and untracked-but-not-ignored files when in a repo.

    Ignored local configuration such as ``local.properties`` is intentionally
    excluded, because it must exist for a Windows developer build and is never
    committed. Outside a git checkout every file is walked.
    """
    listed = git("ls-files", "--cached", "--others", "--exclude-standard")
    if listed:
        return [REPO_ROOT / line for line in listed]

    files: list[Path] = []
    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRECTORIES for part in path.relative_to(REPO_ROOT).parts):
            continue
        files.append(path)
    return files


def check_forbidden_paths(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        relative = path.relative_to(REPO_ROOT)
        parts = set(relative.parts[:-1])

        if relative.name in FORBIDDEN_NAMES:
            problems.append(f"forbidden file present: {relative.as_posix()}")
            continue

        if relative.suffix.lower() in FORBIDDEN_SUFFIXES:
            problems.append(f"forbidden file type present: {relative.as_posix()}")
            continue

        matched_directories = parts & FORBIDDEN_DIRECTORIES
        if matched_directories:
            name = sorted(matched_directories)[0]
            problems.append(f"build or local state directory tracked: {relative.as_posix()} (matched {name}/)")
    return problems


def check_ignore_rules() -> list[str]:
    ignore_file = REPO_ROOT / ".gitignore"
    if not ignore_file.is_file():
        return [".gitignore is missing"]

    lines = {line.strip() for line in ignore_file.read_text(encoding="utf-8").splitlines()}
    return [
        f".gitignore is missing required entry: {entry}"
        for entry in REQUIRED_IGNORE_ENTRIES
        if entry not in lines
    ]


def is_text_file(path: Path) -> bool:
    return path.suffix.lower() in TEXT_SUFFIXES or path.name in {
        ".editorconfig",
        ".gitattributes",
        ".gitignore",
        ".gitmessage",
    }


def check_secret_patterns(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        if not is_text_file(path):
            continue
        try:
            content = path.read_text(encoding="utf-8", errors="ignore")
        except OSError as error:
            problems.append(f"could not read {path.relative_to(REPO_ROOT).as_posix()}: {error}")
            continue

        for label, pattern in SECRET_PATTERNS:
            for match in pattern.finditer(content):
                line_number = content.count("\n", 0, match.start()) + 1
                problems.append(
                    f"possible {label} in {path.relative_to(REPO_ROOT).as_posix()}:{line_number}"
                )
    return problems


def main() -> int:
    files = repository_files()
    problems: list[str] = []
    problems.extend(check_forbidden_paths(files))
    problems.extend(check_ignore_rules())
    problems.extend(check_secret_patterns(files))

    print("Ember REPOSITORY HYGIENE")
    print(f"  Files scanned: {len(files)}")
    print(f"  Forbidden paths checked: {len(FORBIDDEN_NAMES)} names, {len(FORBIDDEN_SUFFIXES)} suffixes")
    print(f"  Secret patterns checked: {len(SECRET_PATTERNS)}")

    if problems:
        print("")
        for problem in problems:
            print(f"  FAIL: {problem}")
        print("")
        print("REPOSITORY HYGIENE: FAIL")
        return 1

    print("")
    print("REPOSITORY HYGIENE: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
