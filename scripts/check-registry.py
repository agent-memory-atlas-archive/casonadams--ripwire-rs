#!/usr/bin/env python3
"""Fail when a plugin file or directory is not known to its registry macro.

`register_languages!` and `register_commands!` expand each `module::...` line into `pub mod module;`,
so a registered module without a file does not compile. The reverse gap is silent: a language or
command directory that no registry line names is never compiled and never registered. This script
checks that direction for both registries.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
LANGUAGES = ROOT / "src" / "languages"
COMMANDS = ROOT / "src" / "commands"
SHARED_LANGUAGE_DIRS = {"common"}
ENTRY = re.compile(r"^\s+(?:\w+ => )?(\w+)::\w+,\s*$")
MOD_LINE = re.compile(r"^\s*(?:pub(?:\([\w:]+\))? )?mod (\w+);")


def macro_modules(text, macro):
    block = text.split(f"{macro}! {{", 1)[1].split("\n}", 1)[0]
    return {m.group(1) for line in block.splitlines() if (m := ENTRY.match(line))}


def declared_modules(text):
    return {m.group(1) for line in text.splitlines() if (m := MOD_LINE.match(line))}


def language_dirs():
    return {
        entry.name
        for entry in LANGUAGES.iterdir()
        if entry.is_dir() and entry.name not in SHARED_LANGUAGE_DIRS and (entry / "mod.rs").is_file()
    }


def command_units():
    units = {entry.stem for entry in COMMANDS.glob("*.rs") if entry.name != "mod.rs"}
    units |= {entry.name for entry in COMMANDS.iterdir() if entry.is_dir() and (entry / "mod.rs").is_file()}
    return units


def check(name, problems):
    for message in problems:
        print(f"::error::{name}: {message}", file=sys.stderr)
    return bool(problems)


def main():
    languages_text = (LANGUAGES / "mod.rs").read_text(encoding="utf-8")
    registered_langs = macro_modules(languages_text, "register_languages")
    dirs = language_dirs()
    failed = check("languages", [
        *(f"src/languages/{d}/mod.rs is not in register_languages!" for d in sorted(dirs - registered_langs)),
        *(f"register_languages! names {d} but src/languages/{d}/mod.rs is missing" for d in sorted(registered_langs - dirs)),
    ])

    commands_text = (COMMANDS / "mod.rs").read_text(encoding="utf-8")
    known = declared_modules(commands_text) | macro_modules(commands_text, "register_commands")
    failed |= check("commands", [
        f"src/commands/{u} is neither declared nor registered in src/commands/mod.rs"
        for u in sorted(command_units() - known)
    ])

    if failed:
        sys.exit(1)
    print(f"registry: {len(registered_langs)} languages and {len(command_units())} command units are registered")


if __name__ == "__main__":
    main()
