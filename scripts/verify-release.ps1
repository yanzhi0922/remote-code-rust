param(
    [switch]$IncludeDesktopBundle,
    [switch]$IncludeAndroid,
    [switch]$IncludeAudit,
    [switch]$SkipFrontend,
    [switch]$SkipRust,
    [switch]$UseProxy,
    [string]$ProxyUrl = "http://127.0.0.1:7890"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

if ($UseProxy) {
    $env:HTTP_PROXY = $ProxyUrl
    $env:HTTPS_PROXY = $ProxyUrl
}

function Run-Step([string]$Name, [scriptblock]$Command) {
    Write-Host ""
    Write-Host "=== $Name ==="
    & $Command
}

Run-Step "Git whitespace" { git diff --check }

if (-not $SkipRust) {
    Run-Step "Rust format" { cargo fmt --all -- --check }
    Run-Step "Control plane check" { cargo check -p remote-code-control-plane --all-targets }
    Run-Step "Runner check" { cargo check -p remote-code-runner --all-targets }
    Run-Step "GUI Rust check" { cargo check -p remote-code-gui --all-targets }
}

if (-not $SkipFrontend) {
    Push-Location (Join-Path $RepoRoot "apps\remote-code-gui")
    try {
        Run-Step "GUI npm build" { npm run build }
        Run-Step "GUI npm test" { npm test }
        if ($IncludeAudit) {
            Run-Step "GUI npm audit" { npm audit --registry=https://registry.npmjs.org --audit-level=moderate }
        }
        if ($IncludeDesktopBundle) {
            Run-Step "Windows desktop bundle" { npm run desktop:build }
        }
        if ($IncludeAndroid) {
            Run-Step "Android mobile bundle" { npm run android:build:debug }
        }
    }
    finally {
        Pop-Location
    }
}

Write-Host ""
Write-Host "Release verification complete."
