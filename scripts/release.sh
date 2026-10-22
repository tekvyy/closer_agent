#!/usr/bin/env bash
# Release a new version: bumps the version everywhere, commits, tags and pushes.
# The tag starts the Release workflow, and installed apps pick the update up from
# its latest.json. Usage: scripts/release.sh 0.3.0
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Usage: $0 <version>   e.g. $0 0.3.0" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Commit or stash your changes first." >&2
  exit 1
fi

current="$(jq -r .version src-tauri/tauri.conf.json)"
echo "Releasing $current -> $version"

tmp="$(mktemp)"
jq --arg v "$version" '.version = $v' src-tauri/tauri.conf.json > "$tmp" && mv "$tmp" src-tauri/tauri.conf.json
jq --arg v "$version" '.version = $v' package.json > "$tmp" && mv "$tmp" package.json
# Only the [package] version, which is the first `version =` line
awk -v v="$version" '!done && /^version = / { print "version = \"" v "\""; done = 1; next } { print }' \
  src-tauri/Cargo.toml > "$tmp" && mv "$tmp" src-tauri/Cargo.toml
bunx prettier --write src-tauri/tauri.conf.json package.json > /dev/null

git add src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml
git commit -m "Release v$version" --trailer "Co-Authored-By: Claude <noreply@anthropic.com>"
# A message, since tag.gpgsign makes every tag annotated
git tag -a "v$version" -m "Release v$version"
git push origin HEAD "v$version"

echo "Pushed v$version. Follow the build in the repo's Actions tab."
