# CI-only release helper for draw.io Tauri
#
# Releases are built, signed, and published by GitHub Actions
# (.github/workflows/build.yml) using the TAURI_SIGNING_PRIVATE_KEY repo
# secret. The private key is intentionally NOT stored on this machine —
# GitHub secrets are write-only, so CI is the single copy.
#
# What this script does locally:
#   1. npm run sync           (drawio/VERSION -> package.json, Cargo.toml, tauri.conf.json)
#   2. git tag vX.Y.Z         (annotated)
#   3. git push origin vX.Y.Z (triggers the CI release pipeline)
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1 [-Push]
#
# Without -Push it only prints the steps (dry run).

param(
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$repo = 'rede97/drawio-tauri'

# ── Sync version (source of truth: drawio/VERSION) ────────────────────────
npm run sync | Out-Null
$version = (Get-Content 'drawio/VERSION').Trim()
$tag = "v$version"

# ── Pre-flight: tag must not already exist on the remote ──────────────────
$existing = git ls-remote --tags origin $tag 2>$null
if ($existing) {
    throw "Tag $tag already exists on origin. Bump drawio/VERSION first (update the submodule)."
}

Write-Host "==> Release candidate: $tag" -ForegroundColor Cyan

if (-not $Push) {
    Write-Host @"

Dry run. To release ${tag}:

    git add -A; git commit -m "release $version"
    git push origin dev
    git tag -a $tag -m "draw.io $version"
    git push origin $tag

The tag push triggers CI: build -> sign NSIS updater artifacts -> generate
latest.json -> publish the GitHub release (clients auto-update afterwards).
Or re-run this script with -Push to execute the tag + push steps.
"@
    exit 0
}

# ── Tag + push ─────────────────────────────────────────────────────────────
$dirty = git status --porcelain
if ($dirty) {
    throw "Working tree is dirty. Commit and push your changes to dev first:`n$dirty"
}

git tag -a $tag -m "draw.io $version"
git push origin $tag
if ($LASTEXITCODE -ne 0) { throw "Failed to push $tag" }

Write-Host "==> $tag pushed. CI is building the release:" -ForegroundColor Green
Write-Host "    https://github.com/$repo/actions"
