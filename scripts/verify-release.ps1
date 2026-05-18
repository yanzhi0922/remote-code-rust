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
    $env:ALL_PROXY = $ProxyUrl
}

function Run-Step([string]$Name, [scriptblock]$Command) {
    Write-Host ""
    Write-Host "=== $Name ==="
    & $Command
}

Run-Step "Git whitespace" { git diff --check }

if (-not $SkipRust) {
    $previousRustMinStack = $env:RUST_MIN_STACK
    $previousRustTestThreads = $env:RUST_TEST_THREADS
    $env:RUST_MIN_STACK = "16777216"
    $env:RUST_TEST_THREADS = "1"
    try {
    Run-Step "Rust format" { cargo fmt --all -- --check }
    Run-Step "Control plane check" { cargo check -p remote-code-control-plane --all-targets }
    Run-Step "Runner check" { cargo check -p remote-code-runner --all-targets }
    Run-Step "GUI Rust check" { cargo check -p remote-code-gui --all-targets }
    Run-Step "Roo CLI check" { cargo check -p roo-cli --bin roo --tests -j 1 }
    Run-Step "Roo task/server/adapter check" { cargo check -p roo-task -p roo-server -p rc-roo-adapter --all-targets -j 1 }
    Run-Step "Roo provider check" { cargo check -p roo-provider-minimax -p roo-provider-openai -p roo-provider-fake-ai --all-targets -j 1 }
    Run-Step "Roo CLI smoke/stress" { & (Join-Path $RepoRoot "scripts\roo-cli-stress.ps1") -BuildIfMissing }
    Run-Step "Roo focused tests" {
        $previousTestDebug = $env:CARGO_PROFILE_TEST_DEBUG
        try {
            $env:CARGO_PROFILE_TEST_DEBUG = "0"
            cargo test -p roo-cli --all-targets -j 1
            cargo test -p roo-prompt --all-targets -j 1
            cargo test -p roo-provider-minimax --all-targets -j 1
            cargo test -p roo-provider-openai --all-targets -j 1
            cargo test -p roo-provider-fake-ai --all-targets -j 1
            cargo test -p roo-task --lib -j 1 message_builder
        }
        finally {
            if ($null -eq $previousTestDebug) {
                Remove-Item Env:\CARGO_PROFILE_TEST_DEBUG -ErrorAction SilentlyContinue
            } else {
                $env:CARGO_PROFILE_TEST_DEBUG = $previousTestDebug
            }
        }
    }
    Run-Step "Rust clippy" { cargo clippy --workspace --all-targets --exclude remote-code-gui -j 1 -- -D warnings }
    if ($IncludeAudit) {
        Run-Step "Rust audit" { cargo audit --quiet --no-fetch }
    }
    }
    finally {
        if ($null -eq $previousRustMinStack) {
            Remove-Item Env:\RUST_MIN_STACK -ErrorAction SilentlyContinue
        } else {
            $env:RUST_MIN_STACK = $previousRustMinStack
        }
        if ($null -eq $previousRustTestThreads) {
            Remove-Item Env:\RUST_TEST_THREADS -ErrorAction SilentlyContinue
        } else {
            $env:RUST_TEST_THREADS = $previousRustTestThreads
        }
    }
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
