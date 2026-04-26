# build-agents.ps1 — Build agent binaries for multi-agent architecture
# Usage: powershell -ExecutionPolicy Bypass -File scripts/build-agents.ps1 [roo-code|codex|all]
param(
    [ValidateSet("roo-code", "codex", "all")]
    [string]$Agent = "all"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$AgentsDir = Join-Path $ProjectRoot "agents"
$OutputBase = Join-Path $ProjectRoot "target" "agent-binaries"

function Build-RooCode {
    Write-Host "`n=== Building Roo Code Agent ===" -ForegroundColor Cyan
    $RooDir = Join-Path $AgentsDir "roo-code"
    if (-not (Test-Path $RooDir)) {
        Write-Error "Roo Code source not found at $RooDir. Clone it first: git clone <roo-code-url> agents/roo-code"
        return $false
    }

    Push-Location $RooDir
    try {
        Write-Host "  Compiling roo-server (release)..."
        cargo build --release -p roo-server 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to build roo-server"
            return $false
        }

        # Copy binary to output
        $BinSrc = Join-Path $RooDir "target" "release" "roo-server.exe"
        $OutDir = Join-Path $OutputBase "roo-code"
        if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
        Copy-Item $BinSrc $OutDir -Force
        Write-Host "  -> Copied to $OutDir\roo-server.exe" -ForegroundColor Green
        return $true
    }
    finally {
        Pop-Location
    }
}

function Build-Codex {
    Write-Host "`n=== Building Codex Agent ===" -ForegroundColor Cyan
    $CodexDir = Join-Path $AgentsDir "codex"
    if (-not (Test-Path $CodexDir)) {
        Write-Error "Codex source not found at $CodexDir. Clone it first: gh repo clone openai/codex agents/codex"
        return $false
    }

    $CodexRsDir = Join-Path $CodexDir "codex-rs"
    if (-not (Test-Path $CodexRsDir)) {
        Write-Error "codex-rs directory not found at $CodexRsDir"
        return $false
    }

    Push-Location $CodexRsDir
    try {
        Write-Host "  Compiling codex-rs/app-server (release)..."
        cargo build --release -p codex-app-server 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to build codex-app-server"
            return $false
        }

        # Copy binary to output
        $BinSrc = Join-Path $CodexRsDir "target" "release" "codex-app-server.exe"
        if (-not (Test-Path $BinSrc)) {
            # Try alternative binary name
            $BinSrc = Join-Path $CodexRsDir "target" "release" "codex.exe"
        }
        $OutDir = Join-Path $OutputBase "codex"
        if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
        Copy-Item $BinSrc $OutDir -Force
        Write-Host "  -> Copied to $OutDir" -ForegroundColor Green
        return $true
    }
    finally {
        Pop-Location
    }
}

# Main
Write-Host "Remote Code — Agent Binary Builder" -ForegroundColor Yellow
Write-Host "Project root: $ProjectRoot"
Write-Host "Output:       $OutputBase"

if (-not (Test-Path $OutputBase)) {
    New-Item -ItemType Directory -Path $OutputBase -Force | Out-Null
}

$success = $true
switch ($Agent) {
    "roo-code" { $success = Build-RooCode }
    "codex"    { $success = Build-Codex }
    "all"      {
        $r = Build-RooCode
        $c = Build-Codex
        $success = ($r -and $c)
    }
}

if ($success) {
    Write-Host "`n✓ All requested agents built successfully!" -ForegroundColor Green
} else {
    Write-Host "`n✗ Some builds failed." -ForegroundColor Red
    exit 1
}
