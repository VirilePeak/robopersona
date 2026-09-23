#!/usr/bin/env python3
"""Offline link check for the generated site (stdlib only).

Fails on:

* root-absolute or protocol-relative links: they break under a project-pages
  subpath such as ``/robopersona/``;
* relative targets that do not exist in the output;
* ``#fragments`` with no matching ``id`` in the target page;
* unreplaced ``{{placeholders}}``.

External http(s) links are counted, not fetched.

Usage: python site/check.py _site
"""

from __future__ import annotations

import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

SKIP_SCHEMES = {"http", "https", "mailto", "data"}


class Page(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.links: list[tuple[int, str]] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        for name, value in attrs:
            if value is None:
                continue
            if name == "id" or (tag == "a" and name == "name"):
                self.ids.add(value)
            elif name in ("href", "src"):
                self.links.append((self.getpos()[0], value))


def parse(path: Path) -> Page:
    page = Page()
    page.feed(path.read_text(encoding="utf-8"))
    page.close()
    return page


def main(out_arg: str) -> int:
    out = Path(out_arg).resolve()
    if not (out / "index.html").is_file():
        print(f"check: {out}/index.html missing")
        return 1
    pages = {p: parse(p) for p in sorted(out.rglob("*.html"))}
    errors: list[str] = []
    internal = external = 0

    for path, page in pages.items():
        rel = path.relative_to(out)
        if "{{" in path.read_text(encoding="utf-8"):
            errors.append(f"{rel}: unreplaced placeholder")
        if path.name == "404.html":
            continue  # served at arbitrary depths; its links are absolute by design
        for line, url in page.links:
            parts = urlsplit(url)
            where = f"{rel}:{line}: {url!r}"
            if parts.scheme in SKIP_SCHEMES:
                external += parts.scheme in ("http", "https")
                continue
            if parts.scheme or parts.netloc or url.startswith("/"):
                errors.append(f"{where} is root-absolute or has an unexpected scheme")
                continue
            internal += 1
            target = path
            if parts.path:
                target = (path.parent / unquote(parts.path)).resolve()
                if target.is_dir():
                    target = target / "index.html"
            if not target.is_relative_to(out):
                errors.append(f"{where} escapes the site root")
            elif not target.is_file():
                errors.append(f"{where} -> missing {target.relative_to(out)}")
            elif parts.fragment:
                if target.suffix != ".html":
                    errors.append(f"{where} has a fragment on a non-HTML target")
                elif unquote(parts.fragment) not in pages[target].ids:
                    errors.append(f"{where} -> no id {parts.fragment!r} in {target.relative_to(out)}")

    # demo.js imports the wasm glue from JS, which the HTML scan cannot see.
    if (out / "demo.js").is_file():
        for name in ("wasm/botpack_wasm_opt.js", "wasm/botpack_wasm_opt_bg.wasm"):
            if not (out / name).is_file():
                errors.append(f"demo.js is present but {name} is missing")

    for e in errors:
        print(f"check: {e}")
    status = "FAILED" if errors else "ok"
    print(
        f"check: {status} — {len(pages)} pages, {internal} internal links, "
        f"{external} external links (not fetched), {len(errors)} errors"
    )
    return 1 if errors else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
