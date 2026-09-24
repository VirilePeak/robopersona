# Reachy Mini example souls

A `.botpack` soul is a directory with a `manifest.json` plus payload files.
For the [Reachy Mini conversation app](https://github.com/pollen-robotics/reachy_mini_conversation_app),
a soul maps onto a **personality profile**: one `profile.md` file with TOML
frontmatter (`schema_version`, optional `voice` and `greeting`, and
`default_tools` — the tool modules the profile may use) followed by the
personality prompt as the document body. The app's loader (`profile_store.py`)
rejects unknown frontmatter fields, so a soul payload contains exactly that
one file.

## What is here

| Soul | Character |
|---|---|
| `nova-companion` | Calm, curious desktop companion. Bilingual (en/de), gentle motion, honest about being a robot. |

## Install

Requires the `botpack` CLI ([releases](https://github.com/VirilePeak/robopersona/releases/latest)
or `cargo install --git https://github.com/VirilePeak/robopersona --locked botpack`)
and a Reachy Mini conversation app checkout.

```console
$ botpack verify examples/reachy/nova-companion.botpack
OK nova-companion v0.1.0 (format 0.1.0) — 1 payload entries
$ ./examples/reachy/install.sh examples/reachy/nova-companion.botpack \
    <checkout>/external_content/user_personalities
installed soul 'nova-companion' -> <checkout>/external_content/user_personalities/nova-companion
```

The script unpacks the soul into the app's user-personalities root, which
must be passed explicitly — it depends on how the app runs: for a source
checkout that is `<checkout>/external_content/user_personalities`, for an
installed app `<instance-path>/user_personalities`. After restarting the
conversation app, the personality appears in the web UI (`--ui`). The
`default_tools` in the frontmatter are the profile's tool defaults; the UI
can override them per profile.

## Integrity

`botpack verify` recomputes every SHA-256 digest in the archive and fails
on any mismatch — a corrupted or truncated download is detected before it
reaches the robot. Note what this does **not** mean: the payload is plain
text after unpacking, and checksums detect corruption, not redistribution.
There is no DRM in `.botpack` 0.1.0.