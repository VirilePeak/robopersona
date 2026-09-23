#!/usr/bin/env python3
"""Build the static project page for RoboPersona / .botpack.

Requires Python >= 3.11 (tomllib) and Markdown (hash-pinned in
``site/requirements.txt``).

The sources of truth stay where they are:

* ``SPEC.md`` is rendered verbatim into ``spec/index.html``;
* ``conformance/affect-reference.json`` is copied unchanged, and its scenario
  and expected bit patterns are injected into the landing page;
* versions come from ``SPEC.md`` (format) and ``Cargo.toml`` (crates).

Nothing is typed twice, so the page cannot drift from the spec or the fixture.

With ``--wasm DIR`` (the ``wasm/`` directory of a release's
``botpack-wasm.tar.gz``) the landing page also runs the conformance scenario
in the visitor's browser and compares the result with the fixture bits.

Every internal link is relative, so the same output works under a
project-pages subpath (``/robopersona/``) and on an apex custom domain.
``--base-url`` only feeds canonical/Open Graph tags and the 404 page, which
GitHub Pages serves at arbitrary depths.

Usage::

    python site/build.py --out _site [--base-url URL] [--wasm DIR --wasm-tag TAG]
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import shutil
import subprocess
import tomllib
from pathlib import Path
from typing import NoReturn

import markdown

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
REPO_URL = "https://github.com/VirilePeak/robopersona"
WASM_FILES = ("botpack_wasm_opt.js", "botpack_wasm_opt_bg.wasm")
PLACEHOLDER = re.compile(r"\{\{\s*([a-z0-9_]+)\s*\}\}")
HEX64 = re.compile(r"[0-9a-f]{16}")
FAVICON = (
    "data:image/svg+xml,"
    "%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E"
    "%3Crect width='32' height='32' rx='7' fill='%230b6e4f'/%3E"
    "%3Ctext x='16' y='21.5' font-family='Menlo,monospace' font-size='14' "
    "font-weight='700' text-anchor='middle' fill='%23ffffff'%3Ebp%3C/text%3E"
    "%3C/svg%3E"
)

DEMO = """<div class="demo" id="demo" hidden>
<p class="demo-title">Computed in your browser by the {tag} release wasm ({size_kb}&nbsp;kB):</p>
<pre class="bits" id="demo-result" aria-live="polite">running…</pre>
</div>
<script type="module" src="demo.js"></script>"""

NOT_FOUND = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Not found — RoboPersona</title>
<style>
body {{ font: 16px/1.6 system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; padding: 0 1.25rem; }}
a {{ color: #0b6e4f; }}
@media (prefers-color-scheme: dark) {{ body {{ background: #0e1012; color: #e7e7e3; }} a {{ color: #52d1a6; }} }}
</style>
</head>
<body>
<h1>Not found</h1>
<p><a href="{home}">RoboPersona / .botpack</a> · <a href="{home}spec/">Specification</a></p>
</body>
</html>
"""


def die(msg: str) -> NoReturn:
    raise SystemExit(f"build: {msg}")


def spec_version(spec: str) -> str:
    m = re.search(r"^\*\*Version:\*\*\s*(\d+\.\d+\.\d+)", spec, re.MULTILINE)
    if m is None:
        die("SPEC.md has no '**Version:** X.Y.Z' line")
    return m.group(1)


def crate_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as f:
        return tomllib.load(f)["workspace"]["package"]["version"]


def commit_sha() -> str:
    sha = os.environ.get("GITHUB_SHA", "").strip()
    if sha:
        return sha
    out = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    )
    return out.stdout.strip()


def fill(template: str, values: dict[str, str]) -> str:
    """Replace ``{{key}}`` placeholders; unknown keys are a hard error."""

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key not in values:
            die(f"unknown placeholder {{{{{key}}}}} in template")
        return values[key]

    return PLACEHOLDER.sub(sub, template)


def wrap_tables(fragment: str) -> str:
    """Make wide tables scroll horizontally instead of breaking the layout."""
    return fragment.replace("<table>", '<div class="table-wrap"><table>').replace(
        "</table>", "</table></div>"
    )


def fixture_values(fx: dict) -> dict[str, str]:
    sc = fx["scenario"]
    cfg = sc["config"]
    exp = fx["expected_state"]
    values = {
        "baseline": cfg["baseline"]["decimal_non_normative"],
        "decay_rate": cfg["decay_rate_decimal_non_normative"],
        "max_step": cfg["max_step_decimal_non_normative"],
        "stimulus": sc["stimulus"]["decimal_non_normative"],
        "dt": sc["tick"]["dt_decimal_non_normative"],
        "dt_bits": sc["tick"]["dt_bits"],
        "tick_count": str(sc["tick"]["count"]),
        "valence_bits": exp["valence_bits"],
        "arousal_bits": exp["arousal_bits"],
        "dominance_bits": exp["dominance_bits"],
    }
    for key in ("dt_bits", "valence_bits", "arousal_bits", "dominance_bits"):
        if not HEX64.fullmatch(values[key]):
            die(f"fixture field {key} is not 16 lowercase hex digits: {values[key]!r}")
    return {k: html.escape(v) for k, v in values.items()}


def layout(
    *,
    title: str,
    description: str,
    body: str,
    depth: int,
    path: str,
    current: str,
    base_url: str,
    versions: dict[str, str],
    sha: str,
) -> str:
    esc = html.escape
    root = "../" * depth
    home = root or "./"
    meta = []
    if base_url:
        url = esc(f"{base_url}/{path}")
        meta.append(f'<link rel="canonical" href="{url}">')
        meta.append(f'<meta property="og:url" content="{url}">')
    spec_current = ' aria-current="page"' if current == "spec" else ""
    extra_meta = "\n".join(meta)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>{esc(title)}</title>
<meta name="description" content="{esc(description)}">
<meta property="og:type" content="website">
<meta property="og:title" content="{esc(title)}">
<meta property="og:description" content="{esc(description)}">
{extra_meta}
<link rel="icon" href="{FAVICON}">
<link rel="stylesheet" href="{root}style.css">
</head>
<body>
<header class="site">
<a class="brand" href="{home}">RoboPersona <span>.botpack</span></a>
<nav aria-label="Primary">
<a href="{root}spec/"{spec_current}>Spec</a>
<a href="{home}#conformance">Conformance</a>
<a href="{home}#quickstart">Quickstart</a>
<a href="{REPO_URL}">GitHub</a>
</nav>
</header>
<main>
{body}
</main>
<footer class="site">
<p>MIT OR Apache-2.0 · format {versions["spec"]} (draft) · reference implementation {versions["crate"]}</p>
<p>Generated from <a href="{REPO_URL}/blob/{sha}/SPEC.md"><code>SPEC.md</code></a> and <a href="{REPO_URL}/blob/{sha}/conformance/affect-reference.json"><code>affect-reference.json</code></a> at <a href="{REPO_URL}/commit/{sha}"><code>{sha[:7]}</code></a>.</p>
</footer>
</body>
</html>
"""


def main() -> None:
    ap = argparse.ArgumentParser(description="Build the RoboPersona project page.")
    ap.add_argument("--out", required=True, type=Path, help="output directory")
    ap.add_argument(
        "--base-url",
        default="",
        help="absolute site URL without trailing slash (e.g. actions/configure-pages base_url)",
    )
    ap.add_argument(
        "--wasm",
        type=Path,
        help="wasm/ directory of a release's botpack-wasm.tar.gz; enables the in-browser check",
    )
    ap.add_argument(
        "--wasm-tag", default="", help="release tag the --wasm files come from (shown on the page)"
    )
    args = ap.parse_args()

    out = args.out.resolve()
    base_url = args.base_url.rstrip("/")
    if base_url and not re.match(r"https?://", base_url):
        die(f"--base-url must be an absolute http(s) URL: {base_url!r}")
    wasm_files: list[Path] = []
    if args.wasm is not None:
        if not args.wasm_tag:
            die("--wasm requires --wasm-tag")
        wasm_files = [args.wasm / name for name in WASM_FILES]
        missing = [str(p) for p in wasm_files if not p.is_file()]
        if missing:
            die(f"--wasm: missing {', '.join(missing)}")
    if out == ROOT or out in ROOT.parents or out == Path.home():
        die(f"refusing to use {out} as output directory")
    if out.exists():
        if any(out.iterdir()) and not (out / "index.html").exists():
            die(f"{out} is not empty and does not look like a previous build")
        shutil.rmtree(out)
    out.mkdir(parents=True)

    spec_src = (ROOT / "SPEC.md").read_text(encoding="utf-8")
    fixture_path = ROOT / "conformance" / "affect-reference.json"
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    versions = {"spec": spec_version(spec_src), "crate": crate_version()}
    sha = commit_sha()
    common = {"base_url": base_url, "versions": versions, "sha": sha}

    demo = ""
    if wasm_files:
        size_kb = round(wasm_files[1].stat().st_size / 1000)
        demo = DEMO.format(tag=html.escape(args.wasm_tag), size_kb=size_kb)

    # Landing page: template + fixture values + versions.
    values = fixture_values(fixture) | {
        "spec_version": versions["spec"],
        "crate_version": versions["crate"],
        "demo": demo,
    }
    landing = fill((SITE / "landing.html").read_text(encoding="utf-8"), values)
    (out / "index.html").write_text(
        layout(
            title="RoboPersona — the .botpack format",
            description=(
                "Open format for embodied-AI behavior packages: integrity-checked "
                "archives and a deterministic affect engine, bit-identical in Rust, "
                "Python and WebAssembly."
            ),
            body=wrap_tables(landing),
            depth=0,
            path="",
            current="home",
            **common,
        ),
        encoding="utf-8",
    )

    # Specification, rendered verbatim; table of contents before the first section.
    md = markdown.Markdown(
        extensions=["tables", "fenced_code", "toc", "sane_lists"],
        extension_configs={"toc": {"permalink": "#", "toc_depth": "2-3"}},
        output_format="html",
    )
    spec_body = wrap_tables(md.convert(spec_src))
    toc_html = getattr(md, "toc", "")  # set by the toc extension during convert()
    toc = (
        '<nav class="spec-toc" aria-label="Contents">'
        f'<p class="toc-title">Contents</p>\n{toc_html}</nav>\n'
    )
    first_h2 = spec_body.find("<h2")
    spec_body = (
        spec_body[:first_h2] + toc + spec_body[first_h2:] if first_h2 >= 0 else toc + spec_body
    )
    (out / "spec").mkdir()
    (out / "spec" / "index.html").write_text(
        layout(
            title=f"Specification — .botpack {versions['spec']}",
            description=(
                f"Normative specification of the .botpack package format, "
                f"version {versions['spec']} (draft)."
            ),
            body=f'<article class="spec">\n{spec_body}\n</article>',
            depth=1,
            path="spec/",
            current="spec",
            **common,
        ),
        encoding="utf-8",
    )

    # Static assets.
    shutil.copy2(SITE / "style.css", out / "style.css")
    (out / "conformance").mkdir()
    shutil.copy2(fixture_path, out / "conformance" / "affect-reference.json")
    if wasm_files:
        (out / "wasm").mkdir()
        for p in wasm_files:
            shutil.copy2(p, out / "wasm" / p.name)
        shutil.copy2(SITE / "demo.js", out / "demo.js")
    home_abs = f"{base_url}/" if base_url else "/"
    (out / "404.html").write_text(NOT_FOUND.format(home=html.escape(home_abs)), encoding="utf-8")

    for p in sorted(out.rglob("*")):
        if p.is_file():
            print(f"  {p.relative_to(out)}  {p.stat().st_size} B")
    print(
        f"build: ok (format {versions['spec']}, crates {versions['crate']}, "
        f"commit {sha[:7]}, base_url {base_url or '-'}, wasm {args.wasm_tag or '-'})"
    )


if __name__ == "__main__":
    main()
