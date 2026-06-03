#!/usr/bin/env pwsh
# purge_zcode_refs.ps1 — Windows equivalent of purge_zcode_refs.sh.
# Same logic: delete refs/zcode/** refs and gc. Use this from Task Scheduler
# or any Windows-side automation.
[CmdletBinding()]
param(
    [switch]$Dry
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$refs = git for-each-ref --format='%(refname)' 'refs/zcode/**' 2>$null
if (-not $refs) {
    Write-Host "No refs/zcode/** refs to purge."
    return
}

$count = @($refs).Count
Write-Host "Found $count refs/zcode/** ref(s):"
$refs | ForEach-Object { Write-Host "  $_" }

if ($Dry) {
    Write-Host ""
    Write-Host "Dry run: no changes made. Re-run without -Dry to apply."
    return
}

foreach ($ref in $refs) {
    if ([string]::IsNullOrWhiteSpace($ref)) { continue }
    git update-ref -d $ref
}

git reflog expire --expire=now --all 2>$null | Out-Null
git gc --prune=now --quiet

$remaining = (git for-each-ref --format='%(refname)' 'refs/zcode/**' 2>$null | Measure-Object).Count
Write-Host ""
Write-Host "Purge complete: $remaining refs/zcode/** remaining."
