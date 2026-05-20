param(
    [switch]$IncludeDesktopBundle,
    [switch]$IncludeAndroid,
    [switch]$IncludeAudit,
    [switch]$IncludeGitleaks,
    [switch]$IncludeWorkspaceTests,
    [switch]$SkipFrontend,
    [switch]$SkipRust,
    [switch]$UseProxy,
    [string]$ProxyUrl = "http://127.0.0.1:7890"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$RepoRootPath = $RepoRoot.Path
Set-Location $RepoRootPath

if ($UseProxy) {
    $env:HTTP_PROXY = $ProxyUrl
    $env:HTTPS_PROXY = $ProxyUrl
    $env:ALL_PROXY = $ProxyUrl
}

function Run-Step([string]$Name, [scriptblock]$Command) {
    Write-Host ""
    Write-Host "=== $Name ==="
    $global:LASTEXITCODE = 0
    & $Command
    if ($global:LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $global:LASTEXITCODE"
    }
}

function Set-EnvDefault([string]$Name, [string]$Value) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($Name))) {
        [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
    }
}

function Invoke-CargoSlice([string]$Kind, [string]$Slice) {
    python (Join-Path $RepoRootPath "scripts\cargo_workspace_slice.py") $Kind $Slice
}

function Invoke-Gitleaks {
    $gitleaks = Get-Command gitleaks -ErrorAction SilentlyContinue
    if ($null -ne $gitleaks) {
        & $gitleaks.Source detect --source $RepoRootPath --redact
        return
    }

    $docker = Get-Command docker -ErrorAction SilentlyContinue
    if ($null -ne $docker) {
        docker run --rm -v "${RepoRootPath}:/repo" zricethezav/gitleaks:latest detect --source /repo --redact
        return
    }

    throw "gitleaks is not installed and Docker is unavailable for the fallback scanner"
}

Run-Step "Git whitespace" { git diff --check }

if (-not $SkipRust) {
    $savedEnv = @{}
    foreach ($name in @(
        "RUST_MIN_STACK",
        "RUST_TEST_THREADS",
        "CARGO_BUILD_JOBS",
        "CARGO_TEST_JOBS",
        "CARGO_INCREMENTAL",
        "CARGO_PROFILE_TEST_DEBUG",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_PROFILE_RELEASE_DEBUG"
    )) {
        $savedEnv[$name] = [Environment]::GetEnvironmentVariable($name)
    }

    Set-EnvDefault "RUST_MIN_STACK" "16777216"
    Set-EnvDefault "RUST_TEST_THREADS" "1"
    Set-EnvDefault "CARGO_BUILD_JOBS" "1"
    Set-EnvDefault "CARGO_TEST_JOBS" "1"
    Set-EnvDefault "CARGO_INCREMENTAL" "0"
    Set-EnvDefault "CARGO_PROFILE_TEST_DEBUG" "0"
    Set-EnvDefault "CARGO_PROFILE_DEV_DEBUG" "0"
    Set-EnvDefault "CARGO_PROFILE_RELEASE_DEBUG" "0"

    try {
        Run-Step "Rust format" { cargo fmt --all -- --check }
        foreach ($slice in @("claude", "codex", "roo", "apps-shared")) {
            Run-Step "Cargo check slice: $slice" { Invoke-CargoSlice "check" $slice }
        }
        foreach ($slice in @("claude", "codex", "roo", "apps-shared")) {
            Run-Step "Cargo clippy slice: $slice" { Invoke-CargoSlice "clippy" $slice }
        }
        if ($IncludeWorkspaceTests) {
            foreach ($slice in @("claude", "codex", "roo", "apps-shared")) {
                Run-Step "Cargo test slice: $slice" { Invoke-CargoSlice "test" $slice }
            }
        }
        if ($IncludeAudit) {
            Run-Step "Rust audit" { cargo audit --quiet }
        }
    }
    finally {
        foreach ($entry in $savedEnv.GetEnumerator()) {
            if ($null -eq $entry.Value) {
                [Environment]::SetEnvironmentVariable($entry.Key, $null, "Process")
            } else {
                [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, "Process")
            }
        }
    }
}

if ($IncludeGitleaks) {
    Run-Step "Gitleaks secret scan" { Invoke-Gitleaks }
}

if (-not $SkipFrontend) {
    Push-Location (Join-Path $RepoRootPath "apps\remote-code-gui")
    try {
        Run-Step "GUI npm ci" { npm ci --registry=https://registry.npmjs.org/ }
        if ($IncludeAudit) {
            Run-Step "GUI npm audit" { npm audit --registry=https://registry.npmjs.org/ --audit-level=moderate }
        }
        Run-Step "GUI npm test" { npm test }
        Run-Step "GUI npm build" { npm run build }
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
