#!/usr/bin/env python3
"""Validates an Ember commit or pull-request title.

Usage:
    python scripts/check_commit_message.py "feat(android): add launch screen"

The rule is Conventional Commits 1.0.0 with the Ember type set. Bodies and
footers are allowed when the input is a full commit message; only the first
line is validated.
"""

from __future__ import annotations

import re
import sys

TYPES = {
    "build",
    "chore",
    "ci",
    "docs",
    "feat",
    "fix",
    "perf",
    "refactor",
    "revert",
    "style",
    "test",
}

TITLE_PATTERN = re.compile(
    r"^(?P<type>[a-z]+)"
    r"(?:\((?P<scope>[a-z0-9][a-z0-9./-]*)\))?"
    r"(?P<breaking>!)?"
    r": (?P<description>\S.*)$"
)

MAX_LENGTH = 100


def validate(message: str) -> list[str]:
    first_line = message.strip().splitlines()[0].strip() if message.strip() else ""

    if not first_line:
        return ["commit title is empty"]

    problems: list[str] = []
    match = TITLE_PATTERN.match(first_line)
    if not match:
        return [
            "commit title must follow Conventional Commits, for example "
            "'feat(android): add launch screen' or 'fix(core): reject blank peer id'"
        ]

    commit_type = match.group("type")
    description = match.group("description")

    if commit_type not in TYPES:
        problems.append(
            f"unknown type '{commit_type}'; choose one of: {', '.join(sorted(TYPES))}"
        )

    if description[0].isupper():
        problems.append("description must start with a lowercase letter")

    if description.endswith("."):
        problems.append("description must not end with a period")

    if len(first_line) > MAX_LENGTH:
        problems.append(f"title is {len(first_line)} characters; keep it at or below {MAX_LENGTH}")

    return problems


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: check_commit_message.py \"<commit or pull request title>\"", file=sys.stderr)
        return 2

    message = sys.argv[1]
    problems = validate(message)

    print("Ember COMMIT MESSAGE")
    print(f"  Title: {message.strip().splitlines()[0] if message.strip() else ''}")

    if problems:
        print("")
        for problem in problems:
            print(f"  FAIL: {problem}")
        print("")
        print("COMMIT MESSAGE: FAIL")
        return 1

    print("")
    print("COMMIT MESSAGE: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
