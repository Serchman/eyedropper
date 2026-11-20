<#
.SYNOPSIS
  Bumps the Krople version, commits, tags and pushes, which starts the
  Release workflow on GitHub (.github/workflows/release.yml).

.EXAMPLE
  .\scripts\release.ps1            # patch bump: 0.2.0 -> 0.2.1
  .\scripts\release.ps1 minor      # 0.2.0 -> 0.3.0
  .\scripts\release.ps1 major      # 0.2.0 -> 1.0.0
  .\scripts\release.ps1 0.4.0-beta.1   # exact version (suffix = pre-release)
  .\scripts\release.ps1 0.2.0 -RetagExisting
      # re-publish a version whose release build failed: moves the existing
      # tag to a new commit (refused if a GitHub Release already exists)

.NOTES
  Must be run on main, up to date with origin, with no uncommitted changes
  other than the version files. Asks for confirmation before pushing.
#>
param(
    [Parameter(Position = 0)]
    [string]$Version = 'patch',
    [switch]$RetagExisting
)

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$app = Join-Path $repo 'krople-test'
$tauriConf = Join-Path $app 'src-tauri\tauri.conf.json'
$cargoToml = Join-Path $app 'src-tauri\Cargo.toml'
$versionFiles = @(
    'krople-test/src-tauri/tauri.conf.json',
    'krople-test/package.json',
    'krople-test/package-lock.json',
    'krople-test/src-tauri/Cargo.toml',
    'krople-test/src-tauri/Cargo.lock'
)

function Fail([string]$message) {
    Write-Host "ERROR: $message" -ForegroundColor Red
    exit 1
}

# Runs a native command and stops if it fails. (Plain $args, not a param
# block, so flags like -m or -f are passed through to the command.)
function Run {
    $exe, $cmdArgs = $args
    & $exe @cmdArgs
    if ($LASTEXITCODE -ne 0) { Fail "'$exe $($cmdArgs -join ' ')' failed (exit $LASTEXITCODE)" }
}

# Runs a native command, returns $true if it exited 0. Output is discarded.
function Succeeds {
    $exe, $cmdArgs = $args
    $old = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & $exe @cmdArgs *> $null } finally { $ErrorActionPreference = $old }
    return ($LASTEXITCODE -eq 0)
}

function Read-Text([string]$path) { [IO.File]::ReadAllText($path) }
function Write-Text([string]$path, [string]$text) {
    [IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding($false)))
}

Set-Location $repo

# --- Preconditions -----------------------------------------------------------

$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne 'main') { Fail "You are on '$branch'. Switch to main first." }

$dirty = @(git status --porcelain | ForEach-Object { $_.Substring(3).Trim('"') } |
    Where-Object { $versionFiles -notcontains $_ })
if ($dirty.Count -gt 0) {
    Fail ("Uncommitted changes outside the version files:`n  " + ($dirty -join "`n  ") +
        "`nCommit or stash them first.")
}

Run git fetch origin main --tags --quiet
$behind = [int](git rev-list --count HEAD..origin/main)
if ($behind -gt 0) { Fail "main is $behind commit(s) behind origin/main. Run 'git pull' first." }

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) { $cargo = $cargo.Source } else { $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path $cargo)) { Fail 'cargo not found. Install Rust (rustup).' }

# --- Work out the new version -----------------------------------------------

$current = ([regex]::Match((Read-Text $tauriConf), '"version"\s*:\s*"([^"]+)"')).Groups[1].Value
if (-not $current) { Fail "Could not read the version from $tauriConf" }

if ($Version -in @('patch', 'minor', 'major')) {
    $m = [regex]::Match($current, '^(\d+)\.(\d+)\.(\d+)')
    if (-not $m.Success) { Fail "Current version '$current' is not x.y.z" }
    $maj = [int]$m.Groups[1].Value; $min = [int]$m.Groups[2].Value; $pat = [int]$m.Groups[3].Value
    switch ($Version) {
        'major' { $maj++; $min = 0; $pat = 0 }
        'minor' { $min++; $pat = 0 }
        'patch' { $pat++ }
    }
    $new = "$maj.$min.$pat"
} elseif ($Version -match '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    $new = $Version
} else {
    Fail "Version must be patch, minor, major or x.y.z[-suffix], got '$Version'"
}

$tag = "v$new"
$tagLocal = Succeeds git rev-parse -q --verify "refs/tags/$tag"
$tagRemote = [bool](git ls-remote --tags origin "refs/tags/$tag")

if ($tagLocal -or $tagRemote) {
    if (-not $RetagExisting) {
        Fail "Tag $tag already exists. Pick another version, or use -RetagExisting if its release build failed."
    }
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Fail '-RetagExisting needs the GitHub CLI (gh) to check for an existing release.' }
    if (Succeeds gh release view $tag) {
        Fail "A GitHub Release for $tag is already published. Bump to a new version instead."
    }
} elseif ($RetagExisting) {
    Write-Host "Tag $tag does not exist yet; -RetagExisting is not needed." -ForegroundColor Yellow
}

Write-Host "Version: $current -> $new" -ForegroundColor Cyan

# --- Update the version files ------------------------------------------------

$text = Read-Text $tauriConf
$text = ([regex]'"version"\s*:\s*"[^"]+"').Replace($text, "`"version`": `"$new`"", 1)
Write-Text $tauriConf $text

$text = Read-Text $cargoToml
$text = ([regex]'(?m)^version\s*=\s*"[^"]+"').Replace($text, "version = `"$new`"", 1)
Write-Text $cargoToml $text

# package.json + package-lock.json
Push-Location $app
try { Run npm version $new --no-git-tag-version --allow-same-version } finally { Pop-Location }

# Cargo.lock: only the app's own entry changes, no network needed.
Push-Location (Join-Path $app 'src-tauri')
try { Run $cargo update --workspace --offline } finally { Pop-Location }

Write-Host ''
git --no-pager diff --stat -- $versionFiles
Write-Host ''

# --- Commit, tag, push ---------------------------------------------------------

$action = 'Push'
if ($tagLocal -or $tagRemote) { $action = 'Push and MOVE existing tag' }
$answer = Read-Host "$action $tag to origin and start the release build? [y/N]"
if ($answer -notmatch '^(y|yes)$') {
    Write-Host 'Stopped. The version files are changed but nothing was committed.' -ForegroundColor Yellow
    Write-Host 'To undo: git checkout -- krople-test' -ForegroundColor Yellow
    exit 1
}

Run git add -- $versionFiles
if (-not (Succeeds git diff --cached --quiet)) {
    Run git commit -m "Release $tag"
} else {
    Write-Host 'Version files already up to date; tagging the current commit.' -ForegroundColor Yellow
}

if ($tagLocal -or $tagRemote) {
    Run git tag -f -a $tag -m "Release $tag"
    Run git push origin main
    Run git push -f origin "refs/tags/$tag"
} else {
    Run git tag -a $tag -m "Release $tag"
    Run git push origin main
    Run git push origin "refs/tags/$tag"
}

$url = (git remote get-url origin) -replace '^git@github\.com:', 'https://github.com/' -replace '\.git$', ''
Write-Host ''
Write-Host "Done. Release build started for $tag." -ForegroundColor Green
Write-Host "Watch it at $url/actions"
