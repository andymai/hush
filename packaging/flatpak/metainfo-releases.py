#!/usr/bin/env python3
"""Write the <releases> of the AppStream metainfo from CHANGELOG.md.

    metainfo-releases.py CHANGELOG.md packaging/io.github.andymai.hush.metainfo.xml

The metainfo gets the newest releases that release-please wrote to the
changelog, each with its date and its change list. The Flatpak build runs this
script on its own source, so the installed metainfo always lists the release
that it holds. Run it by hand to refresh the copy in the repository.
"""

import re
import sys
from xml.sax.saxutils import escape

RELEASES = 10

HEADING = re.compile(r"^## \[?(\d+\.\d+\.\d+)\]?(?:\([^)]*\))? \((\d{4}-\d{2}-\d{2})\)")
SECTION = re.compile(r"^### (.+)$")
ITEM = re.compile(r"^\* (.+)$")


def plain(item):
    """The text of one changelog line, without its scope, links, and commit."""
    item = re.sub(r"^\*\*[^*]+:\*\* ", "", item)
    item = re.sub(r" \(\[[^\]]+\]\([^)]+\)\)", "", item)
    item = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", item)
    item = item.strip()
    return item[:1].upper() + item[1:]


def inline(text):
    """XML-escaped text, with `code` spans as <code>."""
    parts = text.split("`")
    return "".join(
        f"<code>{escape(part)}</code>" if index % 2 else escape(part)
        for index, part in enumerate(parts)
    )


def releases(changelog):
    found = []
    for line in changelog.splitlines():
        heading = HEADING.match(line)
        if heading:
            if len(found) == RELEASES:
                break
            found.append({"version": heading[1], "date": heading[2], "sections": []})
            continue
        if not found:
            continue
        section = SECTION.match(line)
        if section:
            found[-1]["sections"].append((section[1], []))
            continue
        item = ITEM.match(line)
        if item and found[-1]["sections"]:
            found[-1]["sections"][-1][1].append(plain(item[1]))
    return found


def render(found):
    lines = ["  <releases>"]
    for release in found:
        lines.append(
            f'    <release version="{release["version"]}" date="{release["date"]}" type="stable">'
        )
        sections = [(title, items) for title, items in release["sections"] if items]
        if sections:
            lines.append("      <description>")
            for title, items in sections:
                lines.append(f"        <p>{escape(title)}</p>")
                lines.append("        <ul>")
                lines.extend(f"          <li>{inline(item)}</li>" for item in items)
                lines.append("        </ul>")
            lines.append("      </description>")
        lines.append("    </release>")
    lines.append("  </releases>")
    return "\n".join(lines)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    changelog_path, metainfo_path = sys.argv[1:]
    with open(changelog_path, encoding="utf-8") as file:
        found = releases(file.read())
    if not found:
        sys.exit(f"{changelog_path} has no release")
    with open(metainfo_path, encoding="utf-8") as file:
        metainfo = file.read()
    updated, count = re.subn(
        r"  <releases>.*?</releases>", lambda _: render(found), metainfo, flags=re.S
    )
    if count != 1:
        sys.exit(f"{metainfo_path} needs one <releases> element")
    with open(metainfo_path, "w", encoding="utf-8") as file:
        file.write(updated)


if __name__ == "__main__":
    main()
