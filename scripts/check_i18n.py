#!/usr/bin/env python3
"""
Check i18n translation key coverage.

Scans all Rust source files under crates/ui/src/ for t!("...") and tf!("...")
macro invocations, then compares against the keys defined in the JSON language
files under data/lang/.

Usage:
    python3 scripts/check_i18n.py          # Check both en.json and zh-CN.json
    python3 scripts/check_i18n.py --lang en  # Check only en.json
    python3 scripts/check_i18n.py --lang zh-CN  # Check only zh-CN.json
    python3 scripts/check_i18n.py --fix      # Print keys ready to paste
"""

import argparse
import json
import os
import re
import subprocess
import sys

CRATE_DIR = os.path.join(os.path.dirname(__file__), "..", "crates", "ui", "src")
LANG_DIR = os.path.join(os.path.dirname(__file__), "..", "data", "lang")


def extract_source_keys():
    """Extract all translation keys used in Rust source files."""
    src_keys = set()

    # Walk through all .rs files in the UI crate
    for root, dirs, files in os.walk(CRATE_DIR):
        for fname in files:
            if not fname.endswith(".rs"):
                continue
            path = os.path.join(root, fname)
            with open(path) as f:
                content = f.read()

            # Match t!("key")
            for m in re.finditer(r'(?:\b|crate::)t!\(\"([^\"]+)\"', content):
                k = m.group(1)
                if "{" not in k and k.strip():
                    src_keys.add(k)

            # Match tf!("key", ...)
            for m in re.finditer(r'(?:\b|crate::)tf!\(\"([^\"]+)\"', content):
                k = m.group(1)
                if k.strip():
                    src_keys.add(k)

    return src_keys


def load_lang_keys(lang_file):
    """Load keys from a JSON language file."""
    full_path = os.path.join(LANG_DIR, lang_file)
    if not os.path.exists(full_path):
        print(f"  [ERROR] Language file not found: {full_path}")
        return set()
    with open(full_path) as f:
        return set(json.load(f).keys())


def print_section(title, keys, color=""):
    """Print a sorted list of keys with a header."""
    if not keys:
        return
    reset = "\033[0m" if color else ""
    print(f"\n{title} ({len(keys)}):")
    for k in sorted(keys):
        print(f"  {color}{k}{reset}")


def main():
    parser = argparse.ArgumentParser(description="Check i18n translation key coverage")
    parser.add_argument("--lang", choices=["en", "zh-CN", "all"], default="all",
                       help="Language file to check (default: all)")
    parser.add_argument("--fix", action="store_true",
                       help="Print keys in JSON format ready to paste into language files")
    args = parser.parse_args()

    # Determine which language files to check
    lang_files = []
    if args.lang == "all" or args.lang == "en":
        lang_files.append("en.json")
    if args.lang == "all" or args.lang == "zh-CN":
        lang_files.append("zh-CN.json")

    # Extract source keys
    print("Scanning source files for translation keys...")
    src_keys = extract_source_keys()
    print(f"  Found {len(src_keys)} unique keys in source code.")

    all_ok = True

    for lang_file in lang_files:
        lang_label = lang_file.replace(".json", "")
        lang_keys = load_lang_keys(lang_file)
        print(f"\n{'='*60}")
        print(f"  Language file: data/lang/{lang_file}")
        print(f"  Defined keys:  {len(lang_keys)}")
        print(f"{'='*60}")

        missing = src_keys - lang_keys
        extra = lang_keys - src_keys

        if missing:
            all_ok = False
            print_section("  ❌ MISSING (used in source but not in language file)", missing, "\033[31m")
            if args.fix:
                print(f"\n  Keys to add to {lang_file}:")
                json_entries = []
                for k in sorted(missing):
                    json_entries.append(f'  "{k}": "TODO: translate"')
                print(",\n".join(json_entries))
        else:
            print(f"\n  ✅ All {len(src_keys)} source keys are present.")

        if extra:
            print_section("  ⚠️  EXTRA (defined in language file but not used in source)", extra, "\033[33m")
        else:
            print(f"\n  ✅ No unused keys.")

    if all_ok:
        print(f"\n{'='*60}")
        print("  ✅ All checks passed! Every translation key is accounted for.")
        print(f"{'='*60}")
    else:
        print(f"\n{'='*60}")
        print("  Some issues found. Review the output above.")
        print(f"{'='*60}")
        sys.exit(1)


if __name__ == "__main__":
    main()
