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

function Add-ProcessGitConfig([string]$Key, [string]$Value) {
    $count = 0
    $countText = [Environment]::GetEnvironmentVariable("GIT_CONFIG_COUNT")
    if (-not [string]::IsNullOrWhiteSpace($countText)) {
        [void][int]::TryParse($countText, [ref]$count)
    }

    for ($i = 0; $i -lt $count; $i++) {
        $keyName = "GIT_CONFIG_KEY_$i"
        if ([Environment]::GetEnvironmentVariable($keyName) -eq $Key) {
            [Environment]::SetEnvironmentVariable("GIT_CONFIG_VALUE_$i", $Value, "Process")
            return
        }
    }

    [Environment]::SetEnvironmentVariable("GIT_CONFIG_KEY_$count", $Key, "Process")
    [Environment]::SetEnvironmentVariable("GIT_CONFIG_VALUE_$count", $Value, "Process")
    [Environment]::SetEnvironmentVariable("GIT_CONFIG_COUNT", ($count + 1).ToString(), "Process")
}

if ($UseProxy) {
    $env:HTTP_PROXY = $ProxyUrl
    $env:HTTPS_PROXY = $ProxyUrl
    $env:ALL_PROXY = $ProxyUrl
    $env:http_proxy = $ProxyUrl
    $env:https_proxy = $ProxyUrl
    $env:all_proxy = $ProxyUrl
    $env:CARGO_HTTP_PROXY = $ProxyUrl
    if ([string]::IsNullOrWhiteSpace($env:CARGO_HTTP_TIMEOUT)) { $env:CARGO_HTTP_TIMEOUT = "600" }
    if ([string]::IsNullOrWhiteSpace($env:CARGO_NET_RETRY)) { $env:CARGO_NET_RETRY = "10" }
    if ([string]::IsNullOrWhiteSpace($env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL)) { $env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL = "sparse" }
    $loopbackNoProxy = "localhost,127.0.0.1,::1"
    $env:NO_PROXY = if ([string]::IsNullOrWhiteSpace($env:NO_PROXY)) { $loopbackNoProxy } else { "$($env:NO_PROXY),$loopbackNoProxy" }
    $env:no_proxy = if ([string]::IsNullOrWhiteSpace($env:no_proxy)) { $loopbackNoProxy } else { "$($env:no_proxy),$loopbackNoProxy" }
    Add-ProcessGitConfig "http.proxy" $ProxyUrl
    Add-ProcessGitConfig "https.proxy" $ProxyUrl
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

function Invoke-WithoutProxyEnv([scriptblock]$Command) {
    $proxyEnvNames = @("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy")
    $savedProxyEnv = @{}
    foreach ($name in $proxyEnvNames) {
        $savedProxyEnv[$name] = [Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name, $null, "Process")
    }
    try {
        & $Command
    }
    finally {
        foreach ($entry in $savedProxyEnv.GetEnumerator()) {
            [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, "Process")
        }
    }
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

function Cargo-Home {
    $cargoHome = [Environment]::GetEnvironmentVariable("CARGO_HOME")
    if (-not [string]::IsNullOrWhiteSpace($cargoHome)) {
        return $cargoHome
    }
    return (Join-Path $env:USERPROFILE ".cargo")
}

function Get-AdvisoryDbFetchHead {
    $dbPath = Join-Path (Cargo-Home) "advisory-db"
    $fetchHead = Join-Path $dbPath ".git\FETCH_HEAD"
    if (Test-Path $fetchHead) {
        return Get-Item $fetchHead
    }
    return $null
}

function Invoke-CachedCargoAudit {
    $fetchHead = Get-AdvisoryDbFetchHead
    if ($null -eq $fetchHead) {
        Write-Host "cargo audit fetch failed and no cached RustSec advisory DB was found"
        $global:LASTEXITCODE = 1
        return
    }

    $maxAge = [TimeSpan]::FromHours(24)
    $age = (Get-Date) - $fetchHead.LastWriteTime
    if ($age -gt $maxAge) {
        Write-Host "cargo audit fetch failed and cached RustSec advisory DB is older than $($maxAge.TotalHours) hours: $($fetchHead.LastWriteTime.ToString("o"))"
        $global:LASTEXITCODE = 1
        return
    }

    Write-Host "cargo audit fetch failed; using cached RustSec advisory DB last fetched at $($fetchHead.LastWriteTime.ToString("o"))"
    cargo audit --quiet --no-fetch
}

function Invoke-CargoAudit {
    $attempts = 3
    for ($attempt = 1; $attempt -le $attempts; $attempt++) {
        $global:LASTEXITCODE = 0
        cargo audit --quiet
        if ($global:LASTEXITCODE -eq 0) {
            return
        }
        if ($attempt -lt $attempts) {
            Write-Host "cargo audit failed with exit code $global:LASTEXITCODE; retrying in $($attempt * 10) seconds"
            Start-Sleep -Seconds ($attempt * 10)
        }
    }
    Invoke-CachedCargoAudit
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
                Run-Step "Cargo test slice: $slice" {
                    Invoke-WithoutProxyEnv { Invoke-CargoSlice "test" $slice }
                }
            }
        }
        if ($IncludeAudit) {
            Run-Step "Rust audit" { Invoke-CargoAudit }
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
