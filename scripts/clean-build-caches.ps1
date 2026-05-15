param(
    [switch]$Aggressive,
    [switch]$RemoveReleaseArtifacts
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

$paths = @(
    "apps\remote-code-gui\src-tauri\gen",
    "apps\remote-code-gui\dist",
    "target\deploy"
)

if ($Aggressive) {
    $paths += @(
        "apps\remote-code-gui\node_modules\.vite",
        "apps\remote-code-gui\node_modules\.cache",
        "target\debug\build",
        "target\debug\deps",
        "target\debug\incremental",
        "target\release\build",
        "target\release\deps",
        "target\release\incremental"
    )
}

if ($RemoveReleaseArtifacts) {
    $paths += @(
        "target\release\remote-code-gui.exe",
        "target\release\remote-code-control-plane.exe",
        "target\release\remote-code-control-plane",
        "target\release\bundle"
    )
}

foreach ($path in $paths) {
    $fullPath = Join-Path $RepoRoot $path
    if (Test-Path -LiteralPath $fullPath) {
        Write-Host "Removing $path"
        Remove-Item -LiteralPath $fullPath -Recurse -Force -ErrorAction Stop
    }
}

Get-ChildItem -Path (Join-Path $RepoRoot "target") -Recurse -Include "*.pdb", "*.d" -File -ErrorAction SilentlyContinue |
    Where-Object {
        -not $_.FullName.Contains("\target\release\bundle\")
    } |
    ForEach-Object {
        Write-Host "Removing $($_.FullName.Substring($RepoRoot.Path.Length + 1))"
        Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
    }

Write-Host "Cache cleanup complete."
