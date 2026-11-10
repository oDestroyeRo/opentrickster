"""Rewrite the literals that a build pins in source.

The sites come from `derive.rust_sites`, the table that `check --rust` verifies, so a literal that
`check` accepts is a literal that `repin` can find. Values are replaced where they sit. Layout,
comments and digit separators around them stay as they are.
"""

import re

from . import derive
from .wasm import Malformed

SEPARATOR = re.compile(r"(?<=\d)_(?=\d)")


def read_source(path):
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


def write_source(path, text):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def flatten(text):
    """`text` without digit separators (`9_348_436` reads `9348436`), and the offset in `text`
    of every character that remains."""
    dropped = {m.start() for m in SEPARATOR.finditer(text)}
    where = [i for i in range(len(text)) if i not in dropped]
    return "".join(text[i] for i in where), where


def show(value, was):
    """`value` as source text. A number that was written with separators keeps them."""
    text = str(value)
    if "_" in was and text.isdigit():
        head = len(text) % 3 or 3
        return "_".join([text[:head]] + [text[i:i + 3] for i in range(head, len(text), 3)])
    return text


def pairs(old, new):
    """(old site, new site) for every literal that moves from manifest `old` to manifest `new`,
    and notes for the pins that stay."""
    notes = []
    newer = {site.key: site for site in derive.rust_sites(new)}
    out = []
    for site in derive.rust_sites(old):
        other = newer.pop(site.key, None)
        if other is not None:
            out.append((site, other))
        elif site.optional:
            notes.append("KEPT      %s: %s (the new build carries no catalog JSON)" % (site.file, site.key))
        else:
            raise Malformed("the new build has no value for the pin " + site.key)
    for key, site in newer.items():
        if not site.optional:
            raise Malformed("the old pins have no value for the pin " + key)
        notes.append("KEPT      %s: %s (the old pins carry no reviewed value)" % (site.file, key))
    out.extend(zip(derive.follower_sites(old["build"]), derive.follower_sites(new["build"])))
    return out, notes


def files(old, new):
    """Names of the source files that `plan` reads."""
    return sorted({site.file for pair in pairs(old, new)[0] for site in pair})


def splice(name, text, items):
    """`text` with each (start, end, value, key) of `items` replaced. Two edits may not overlap."""
    items = sorted(items)
    for (_, end, _, key), (start, _, _, other) in zip(items, items[1:]):
        if end > start:
            raise Malformed("%s: the %s and %s pins overlap" % (name, key, other))
    for start, end, value, _ in reversed(items):
        text = text[:start] + value + text[end:]
    return text


class Plan:
    """What a repin would write: new text per changed file, the pins it moves, the pins it keeps."""

    def __init__(self):
        self.texts = {}
        self.moved = {}
        self.notes = []


def plan(old, new, texts):
    """Replace the values of manifest `old` by those of manifest `new` in `texts` ({file: text}).

    Every pin must appear exactly once. A build string that only follows the build, and holds
    another build, stays. Every match is made on the text as it was given, before any
    replacement, so a new value that equals another pin's old value is never replaced twice.
    Nothing is written: the caller decides."""
    both, notes = pairs(old, new)
    flat = {name: flatten(text) for name, text in texts.items()}
    result = Plan()
    result.notes.extend(notes)
    edits = {}
    for was, now in both:
        text = texts[was.file]
        stripped, where = flat[was.file]
        found = list(was.grouped().finditer(stripped))
        if len(found) != 1:
            if was.follows and not found:
                result.notes.append("KEPT      %s: %s holds another build" % (was.file, was.key))
                continue
            raise Malformed("%s holds %d matches of the old %s pin, expected exactly one: %s"
                            % (was.file, len(found), was.key, was.pattern()[:90]))
        for slot in was.values:
            first, last = found[0].span(slot)
            start, end = where[first], where[last - 1] + 1
            edits.setdefault(was.file, []).append((start, end, show(now.values[slot], text[start:end]), was.key))
    for name, items in edits.items():
        text = splice(name, texts[name], items)
        if text != texts[name]:
            result.texts[name] = text
            result.moved[name] = list(dict.fromkeys(
                key for start, end, value, key in items if value != texts[name][start:end]))
    return result
