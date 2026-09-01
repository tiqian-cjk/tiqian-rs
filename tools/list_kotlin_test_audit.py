#!/usr/bin/env python3
"""List changed Kotlin engine test files for the recorded upstream audit range.

The script reads only Git metadata from a neighbouring tiqian checkout. It avoids
shell pipelines so the test-audit inventory can be regenerated with one short,
repeatable command before every review batch.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

BASE_REVISION = "2fae0df461819932dc9ef0153b79be9ad0038959"
HEAD_REVISION = "59fca3597a072362c49ce1bade6401efc2d6063d"
TEST_SOURCE_SETS = {
    "commonTest",
    "jvmTest",
    "androidHostTest",
    "jsTest",
    "linuxX64Test",
    "nativeTest",
}


def parse_args() -> argparse.Namespace:
    workspace = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tiqian-root",
        type=Path,
        default=workspace / "tiqian",
        help="Kotlin tiqian checkout (default: sibling workspace checkout)",
    )
    parser.add_argument("--base", default=BASE_REVISION, help="inclusive audit baseline commit")
    parser.add_argument("--head", default=HEAD_REVISION, help="exclusive audit head commit")
    parser.add_argument(
        "--group",
        choices=(
            "fixtures-and-recorded-evidence",
            "foundation-clreq-core-font",
            "punctuation-geometry-and-inline-layout",
            "shaping-and-unicode-boundaries",
            "line-breaking-and-adjustment",
        ),
        help="limit output to one audit work group",
    )
    parser.add_argument("--paths", action="store_true", help="print every test path after the summary")
    return parser.parse_args()


def git(root: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def changed_paths(root: Path, base: str, head: str) -> list[tuple[str, str]]:
    raw = git(
        root,
        "diff",
        "--name-status",
        "-z",
        f"{base}..{head}",
        "--",
        "engine/src",
    ).split(b"\0")
    entries: list[tuple[str, str]] = []
    index = 0
    while index < len(raw) - 1:
        status = raw[index].decode("utf-8")
        index += 1
        if status.startswith(("R", "C")):
            index += 1  # Old path; the destination is the path to audit.
        path = raw[index].decode("utf-8")
        index += 1
        if path.startswith("engine/src/"):
            entries.append((status, path))
    return entries


def source_set(path: str) -> str | None:
    parts = path.split("/")
    return parts[2] if len(parts) > 2 and parts[2] in TEST_SOURCE_SETS else None


def package_group(path: str) -> str:
    parts = path.split("/")
    if len(parts) < 7:
        return "other"
    package = parts[6]
    return package if package in {"clreq", "core", "font", "layout", "linebreak", "shaping", "test", "trace"} else "other"


def audit_group(path: str) -> str:
    if "/resources/golden/" in path:
        return "fixtures-and-recorded-evidence"
    if "/org/tiqian/test/" in path or "/org/tiqian/trace/" in path:
        return "fixtures-and-recorded-evidence"
    if "/org/tiqian/clreq/" in path or "/org/tiqian/core/" in path or "/org/tiqian/font/" in path:
        return "foundation-clreq-core-font"
    if "/org/tiqian/linebreak/" in path:
        return "foundation-clreq-core-font"
    if "/org/tiqian/shaping/" in path:
        return "shaping-and-unicode-boundaries"
    name = Path(path).name
    if name.startswith((
        "AnnotationGeometry",
        "AsciiPointMark",
        "AttachedInline",
        "AutoSpace",
        "Baseline",
        "Bilingual",
        "BopomofoLayout",
        "InlineBox",
        "InlineObject",
        "Interpunct",
        "OpeningBracket",
        "Punctuation",
        "PushInLineWide",
        "R3Geometry",
        "Ruby",
        "SpacingAndLineGeometry",
        "VerbatimRange",
    )):
        return "punctuation-geometry-and-inline-layout"
    if name.startswith((
        "ClusterRole",
        "Contextual",
        "DisplayGlyph",
        "FontInstanceMetrics",
        "ParagraphShaping",
        "Quote",
        "UnicodeEmoji",
        "UnicodePunctuation",
        "WidthIndependentAnnotation",
    )):
        return "shaping-and-unicode-boundaries"
    if name.startswith((
        "DecideHyphen",
        "EmergencyGrapheme",
        "ExplainableStubParagraph",
        "Greedy",
        "HyphenationLayout",
        "Justifier",
        "Kinsoku",
        "LineAdjustment",
        "LineBreak",
        "LineCandidate",
        "LineGeometry",
        "LineOptimization",
        "LineRepair",
        "Lookahead",
        "ParagraphDp",
        "ParagraphLayout",
        "ProgressiveBreak",
        "ProgressiveTechnical",
        "ZeroWidthBreak",
    )):
        return "line-breaking-and-adjustment"
    if name.startswith((
        "GiantTokenScaling",
        "LayoutDump",
        "LayoutReport",
        "PreparedParagraph",
        "RecordedEvidence",
        "ShapingEvidenceRecorder",
    )):
        return "fixtures-and-recorded-evidence"
    return "unclassified"


def main() -> int:
    args = parse_args()
    root = args.tiqian_root.resolve()
    if not (root / ".git").exists():
        print(f"Kotlin checkout is not a Git worktree: {root}", file=sys.stderr)
        return 2
    try:
        entries = changed_paths(root, args.base, args.head)
    except subprocess.CalledProcessError as error:
        print(error.stderr.decode("utf-8", errors="replace").rstrip(), file=sys.stderr)
        return error.returncode or 1

    tests = [(status, path) for status, path in entries if source_set(path)]
    if args.group:
        tests = [entry for entry in tests if audit_group(entry[1]) == args.group]
    source_sets = Counter(source_set(path) for _, path in tests)
    groups: dict[str, list[tuple[str, str]]] = defaultdict(list)
    audit_groups: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for entry in tests:
        groups[package_group(entry[1])].append(entry)
        audit_groups[audit_group(entry[1])].append(entry)

    print(f"audit range: {args.base}..{args.head}")
    print(f"engine/src changed paths: {len(entries)}")
    print(f"test paths: {len(tests)}")
    if args.group:
        print(f"selected audit group: {args.group}")
    print("source sets:")
    for name in sorted(source_sets):
        print(f"  {name}: {source_sets[name]}")
    print("package groups:")
    for name in sorted(groups):
        print(f"  {name}: {len(groups[name])}")
    print("audit groups:")
    for name in sorted(audit_groups):
        print(f"  {name}: {len(audit_groups[name])}")
    if "unclassified" in audit_groups:
        print("unclassified paths:")
        for status, path in sorted(audit_groups["unclassified"], key=lambda entry: entry[1]):
            print(f"  {status}\t{path}")

    if args.paths:
        print("paths:")
        for name in sorted(audit_groups):
            print(f"  [{name}]")
            for status, path in sorted(audit_groups[name], key=lambda entry: entry[1]):
                print(f"    {status}\t{path}")
    return 1 if "unclassified" in audit_groups else 0


if __name__ == "__main__":
    raise SystemExit(main())
