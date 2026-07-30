<#
.SYNOPSIS
    Publish one cloudshell version for Windows, macOS and Linux.

.DESCRIPTION
    Creates the release commit and annotated Git tag locally. Pushing the tag
    starts .github/workflows/release.yml, whose native GitHub runners build all
    platform archives, MSI/AppImage where available, and attach them to the
    GitHub Release.

.EXAMPLE
    .\scripts\release.ps1 v0.4.13 -Push
    .\scripts\release.ps1 0.4.13 -DryRun
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidatePattern('^v?\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')]
    [string] $Version,

    [switch] $Push,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
$tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
$versionNumber = $tag.Substring(1)

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]] $Arguments)
    if ($DryRun) { Write-Host "git $($Arguments -join ' ')"; return }
    & git @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') failed" }
}

function Invoke-Cargo {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]] $Arguments)
    if ($DryRun) { Write-Host "cargo $($Arguments -join ' ')"; return }
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "cargo $($Arguments -join ' ') failed" }
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) {
    throw 'Run this script inside the cloudshell git repository.'
}
Set-Location $repoRoot

if (-not $DryRun) {
    & git diff --quiet --exit-code
    if ($LASTEXITCODE -ne 0) { throw 'Unstaged tracked changes exist. Commit or stash them first.' }
    & git diff --cached --quiet --exit-code
    if ($LASTEXITCODE -ne 0) { throw 'Staged changes exist. Commit or stash them first.' }
    if (& git tag --list $tag) { throw "Tag '$tag' already exists." }
}

$cargoToml = Join-Path $repoRoot 'Cargo.toml'
$toml = Get-Content -LiteralPath $cargoToml -Raw
$updated = [regex]::Replace(
    $toml,
    '(?ms)^(\[package\]\s+.*?^version\s*=\s*")[^"]+(".*)$',
    "`${1}$versionNumber`${2}",
    1
)
if ($updated -eq $toml) { throw 'Could not update [package].version in Cargo.toml.' }

if ($DryRun) {
    Write-Host "Would set cloudshell version to $versionNumber and create $tag."
} else {
    Set-Content -LiteralPath $cargoToml -Value $updated -NoNewline
}

# Do not edit Cargo.lock with regex. Cargo owns its root-package entry and
# refreshes it safely for us after the manifest version changes.
Invoke-Cargo check
Invoke-Cargo check --locked

if (-not $DryRun) {
    $reported = (& cargo run --quiet --locked -- --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $reported -ne "cloudshell $versionNumber") {
        throw "Version check failed: expected 'cloudshell $versionNumber', got '$reported'."
    }
}

Invoke-Git add Cargo.toml Cargo.lock
Invoke-Git commit -m "Release $tag"
Invoke-Git tag -a $tag -m "Release $tag"

if ($Push) {
    Invoke-Git push origin HEAD
    Invoke-Git push origin $tag
    Write-Host "Released $tag. GitHub Actions is now building Windows, macOS and Linux packages."
} else {
    Write-Host "Created $tag locally. Push with: git push origin HEAD; git push origin $tag"
}
