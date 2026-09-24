#!/bin/sh
# Install a .botpack soul as a Reachy Mini conversation app personality.
# Usage: ./install.sh <soul.botpack> <user-personalities-root>
#
# The app scans two roots for profiles: its packaged profiles/ directory and
# a writable user-personalities root. User-installed souls belong in the
# latter. The root depends on how the app runs, so it must be passed
# explicitly:
#   source checkout:  <checkout>/external_content/user_personalities
#   installed app:    <instance-path>/user_personalities
# A profile is any directory containing a profile.md with TOML frontmatter.
set -e

PACK="$1"
ROOT="$2"

if [ -z "$PACK" ] || [ -z "$ROOT" ]; then
    echo "usage: $0 <soul.botpack> <user-personalities-root>" >&2
    echo "  source checkout:  <checkout>/external_content/user_personalities" >&2
    echo "  installed app:    <instance-path>/user_personalities" >&2
    exit 2
fi
if [ ! -f "$PACK" ]; then
    echo "error: $PACK not found" >&2
    exit 1
fi
if ! command -v botpack >/dev/null 2>&1; then
    echo "error: botpack CLI not found — see https://github.com/VirilePeak/robopersona/releases/latest" >&2
    exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 not found (needed to read the manifest)" >&2
    exit 1
fi

NAME="$(botpack show "$PACK" | python3 -c 'import json,sys; print(json.load(sys.stdin)["name"])')"
if [ -z "$NAME" ]; then
    echo "error: could not read package name from $PACK" >&2
    exit 1
fi
# The Reachy app allows profile names matching [a-zA-Z0-9_-]+; anything else
# (notably ../) must never be joined onto the install root.
case "$NAME" in
    *[!a-zA-Z0-9_-]*|"")
        echo "error: package name '$NAME' is not a valid personality name" >&2
        exit 1
        ;;
esac

DEST="$ROOT/$NAME"
if [ -e "$DEST" ]; then
    echo "error: $DEST already exists — remove it first to reinstall" >&2
    exit 1
fi

mkdir -p "$ROOT"
botpack unpack "$PACK" -o "$DEST"

if [ ! -f "$DEST/profile.md" ]; then
    echo "error: $DEST has no profile.md — this soul does not map to a" >&2
    echo "       conversation-app personality; remove $DEST" >&2
    exit 1
fi

echo "installed soul '$NAME' -> $DEST"
echo "restart the conversation app; the personality appears in the web UI (--ui)."