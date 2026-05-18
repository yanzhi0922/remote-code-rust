param(
    [switch]$Aggressive,
    [switch]$RemoveReleaseArtifacts,
    [switch]$RemoveCargoDepInfo,
    [switch]$RemoveGeneratedTauriArtifacts
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

function Remove-WorkspacePath([string]$RelativePath, [switch]$Recurse) {
    $fullPath = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path -LiteralPath $fullPath)) {
        return
    }

    $resolved = (Resolve-Path -LiteralPath $fullPath).Path
    if (-not $resolved.StartsWith($RepoRoot.Path, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove path outside repository: $resolved"
    }

    Write-Host "Removing $RelativePath"
    if ($Recurse) {
        Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction Stop
    } else {
        Remove-Item -LiteralPath $resolved -Force -ErrorAction Stop
    }
}

$paths = @(
    "apps\remote-code-gui\dist",
    "target\deploy"
)

if ($RemoveGeneratedTauriArtifacts) {
    $paths += "apps\remote-code-gui\src-tauri\gen"
}

if ($Aggressive) {
    $paths += @(
        "apps\remote-code-gui\node_modules\.vite",
        "apps\remote-code-gui\node_modules\.cache",
        "target\debug\build",
        "target\debug\deps",
        "target\debug\incremental",
        "target\release\build",
        "target\release\deps",
        "target\release\incremental",
        "crates\codex\target\debug\build",
        "crates\codex\target\debug\deps",
        "crates\codex\target\debug\incremental",
        "crates\codex\target\release\build",
        "crates\codex\target\release\deps",
        "crates\codex\target\release\incremental"
    )
}

if ($RemoveReleaseArtifacts) {
    $paths += @(
        "target\release\remote-code-gui.exe",
        "target\release\remote-code-control-plane.exe",
        "target\release\remote-code-control-plane",
        "target\release\bundle",
        "crates\codex\target\release\bundle"
    )
}

foreach ($path in $paths) {
    Remove-WorkspacePath $path -Recurse
}

$staleFilePatterns = @("*.pdb")
if ($RemoveCargoDepInfo) {
    $staleFilePatterns += "*.d"
}

$staleRoots = @("target", "crates\codex\target")
foreach ($staleRoot in $staleRoots) {
    $fullStaleRoot = Join-Path $RepoRoot $staleRoot
    if (-not (Test-Path -LiteralPath $fullStaleRoot)) {
        continue
    }
    $resolvedStaleRoot = (Resolve-Path -LiteralPath $fullStaleRoot).Path
    if (-not $resolvedStaleRoot.StartsWith($RepoRoot.Path, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to scan path outside repository: $resolvedStaleRoot"
    }

    Get-ChildItem -Path $resolvedStaleRoot -Recurse -Include $staleFilePatterns -File -ErrorAction SilentlyContinue |
    Where-Object {
        -not $_.FullName.Contains("\target\release\bundle\")
    } |
    ForEach-Object {
        Write-Host "Removing $($_.FullName.Substring($RepoRoot.Path.Length + 1))"
        Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "Cache cleanup complete."
