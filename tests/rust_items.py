"""Find a Rust function by name, wherever it lives under `crates/` (#4026).

Tests that paste a real function into a Rust fixture ask for it by name, not by file path,
so moving the function to another file breaks no test.
"""
from functools import lru_cache
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


@lru_cache(maxsize=None)
def _sources(under):
    return tuple((path, path.read_text(encoding="utf-8"))
                 for path in sorted((ROOT / under).rglob("*.rs")) if "target" not in path.parts)


def rust_fn(name, under="crates"):
    """The one function `name` defined under `under`, from its `fn` to its closing brace.

    Its visibility is dropped (`pub(in crate::game) fn x(` comes back as `fn x(`), so a
    fixture can paste it into its own `impl`. A name defined twice, or not at all, fails.
    """
    pattern = re.compile(r"^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?((?:(?:const|async|unsafe)[ \t]+)*fn[ \t]+"
                         + re.escape(name) + r"\b)", re.M)
    found = [(path, text, match) for path, text in _sources(under) for match in pattern.finditer(text)]
    if len(found) != 1:
        places = [f"{path}:{text.count(chr(10), 0, match.start()) + 1}"
                  for path, text, match in found]
        raise LookupError(f"expected one `fn {name}` under {under}, found {len(found)}: {places}")
    _, text, match = found[0]
    end = text.index("{", match.start(1)) + 1
    depth = 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return "    " + text[match.start(1):end]
