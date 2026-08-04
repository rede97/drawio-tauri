# Release script for draw.io Tauri (Windows)
#
# Builds the app, signs the NSIS bundle for the auto-updater, generates
# latest.json, and publishes everything to a GitHub release via `gh`.
#
# Prerequisites:
#   - gh CLI authenticated (gh auth login), repo + workflow scopes
#   - Signing private key at $env:USERPROFILE\.tauri\drawio-tauri.key
#     (generated once via: cargo tauri signer generate -w ~/.tauri/drawio-tauri.key)
#   - Node.js >= 20 and Rust stable on PATH
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1 [-Draft]
#
# The updater endpoint in tauri.conf.json points to:
#   https://github.com/rede97/drawio-tauri/releases/latest/download/latest.json
# so publishing (non-draft) a newer version makes it available to all clients.

param(
    [switch]$Draft
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$repo   = 'rede97/drawio-tauri'
$keyDir = Join-Path $env:USERPROFILE '.tauri'
$keyFile = Join-Path $keyDir 'drawio-tauri.key'

# ── Pre-flight checks ──────────────────────────────────────────────────────
if (-not (Test-Path $keyFile)) {
    throw "Signing key not found: $keyFile`nGenerate it once with: cargo tauri signer generate -w ~/.tauri/drawio-tauri.key"
}
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh CLI not found on PATH"
}
gh auth status | Out-Null

# ── Version (source of truth: drawio/VERSION) ─────────────────────────────
npm run sync | Out-Null
$version = (Get-Content 'drawio/VERSION').Trim()
$tag = "v$version"
Write-Host "==> Releasing $tag" -ForegroundColor Cyan

# ── Build + sign (tauri picks up the signing env vars automatically) ───────
# createUpdaterArtifacts is injected via --config so plain dev builds without
# the private key keep working.
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyFile -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''

$overrideFile = Join-Path $env:TEMP 'tauri-updater-override.json'
'{"bundle":{"createUpdaterArtifacts":true}}' | Set-Content $overrideFile -Encoding utf8

cargo tauri build --bundles nsis -c $overrideFile
if ($LASTEXITCODE -ne 0) { throw 'cargo tauri build failed' }

$setupExe = "src-tauri/target/release/bundle/nsis/draw.io_${version}_x64-setup.exe"
$sigFile  = "$setupExe.sig"
if (-not (Test-Path $setupExe)) { throw "Bundle not found: $setupExe" }
if (-not (Test-Path $sigFile))  { throw "Signature not found: $sigFile (was TAURI_SIGNING_PRIVATE_KEY picked up?)" }

# ── latest.json (updater manifest) ─────────────────────────────────────────
$signature = (Get-Content $sigFile -Raw).Trim()
$pubDate = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
$latest = [ordered]@{
    version  = $version
    notes    = "See https://github.com/$repo/releases/tag/$tag"
    pub_date = $pubDate
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = $signature
            url       = "https://github.com/$repo/releases/download/$tag/draw.io_${version}_x64-setup.exe"
        }
    }
}
$latestPath = 'src-tauri/target/release/bundle/latest.json'
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath -Encoding utf8
Write-Host "==> latest.json written" -ForegroundColor Cyan

# ── Publish via gh ─────────────────────────────────────────────────────────
$assets = @($setupExe, $sigFile, $latestPath)
$existing = gh release view $tag --repo $repo 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-Host "==> Release $tag exists, uploading assets (clobber)" -ForegroundColor Yellow
    gh release upload $tag @assets --repo $repo --clobber
} else {
    $createArgs = @('release', 'create', $tag, '--repo', $repo,
                    '--title', "draw.io $version", '--generate-notes')
    if ($Draft) { $createArgs += '--draft' }
    gh @createArgs @assets
}
if ($LASTEXITCODE -ne 0) { throw 'gh release failed' }

if ($Draft) {
    Write-Host "==> Draft release $tag created. Publish it on GitHub to ship the update." -ForegroundColor Green
} else {
    Write-Host "==> Release $tag published. Clients will auto-update on next startup." -ForegroundColor Green
}
