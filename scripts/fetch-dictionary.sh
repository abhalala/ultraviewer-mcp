#!/usr/bin/env bash
# Download the official UltraViewer installer from ultraviewer.net, extract only
# app/8bit.dat (the installer is never run), and install it with
# `ultraviewer-mcp install-dictionary`, which checks the pinned SHA-256.
#
# Usage: scripts/fetch-dictionary.sh [--binary PATH] [--allow-stale-tls]
#
# Needs curl and innoextract (brew install innoextract / apt install innoextract).
# --allow-stale-tls skips certificate checks on the download only. Nothing
# downloaded is executed, and the extracted file is rejected unless its SHA-256
# matches, so use it only when ultraviewer.net's certificate has lapsed.
set -euo pipefail

page_url="https://www.ultraviewer.net/en/download.html"
fallback_installer="UltraViewer_setup_6.6_en.exe"
binary=""
curl_tls=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --allow-stale-tls) curl_tls=(--insecure); shift ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$binary" ]]; then
  if command -v ultraviewer-mcp >/dev/null; then
    binary="$(command -v ultraviewer-mcp)"
  else
    binary="$(cd "$(dirname "$0")/.." && pwd)/target/release/ultraviewer-mcp"
  fi
fi
[[ -x "$binary" ]] || { echo "ultraviewer-mcp binary not found; build it or pass --binary PATH" >&2; exit 1; }
command -v innoextract >/dev/null || { echo "innoextract is required (brew install innoextract, or apt install innoextract)" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fetch() { curl -fsSL --proto '=https' -m 300 -A "ultraviewer-mcp-dictionary-fetch" ${curl_tls[@]+"${curl_tls[@]}"} "$@"; }

if ! fetch "$page_url" -o "$work/page.html"; then
  echo "Could not reach $page_url over HTTPS. If its certificate has lapsed, rerun with --allow-stale-tls," >&2
  echo "or copy 8bit.dat from an UltraViewer install folder on Windows and run: $binary install-dictionary <path>" >&2
  exit 1
fi
installer="$(grep -oE 'UltraViewer_setup_[0-9.]+_en\.exe' "$work/page.html" | head -n1 || true)"
installer="${installer:-$fallback_installer}"

echo "Downloading $installer from ultraviewer.net"
fetch "https://www.ultraviewer.net/en/$installer" -o "$work/setup.exe"

mkdir "$work/out"
innoextract --silent --output-dir "$work/out" --include app/8bit.dat "$work/setup.exe"
[[ -f "$work/out/app/8bit.dat" ]] || { echo "8bit.dat was not found in $installer" >&2; exit 1; }

"$binary" install-dictionary "$work/out/app/8bit.dat"
