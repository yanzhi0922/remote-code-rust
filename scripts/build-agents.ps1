#Requires -Version 7.0
<#
.SYNOPSIS
    Build all three Agent binaries for the multi-agent architecture.

.DESCRIPTION
    Builds Claude Code Agent, Codex Agent, and Roo-code Agent, then copies
    the resulting binaries into a unified output directory.
    Local development / trusted runner machines only. Do not run this on the
    relay-only cloud control-plane host.

.EXAMPLE
    ./scripts/build-agents.ps1
    ./scripts/build-agents.ps1 -Release
    ./scripts/build-agents.ps1 -Debug
#>

param(
    [switch]$Release = $true,
    [switch]$Debug = $false
)

$ErrorActionPreference = "Stop"

# Determine build profile
$Profile = if ($Debug) { "debug" } else { "release" }
$ProfileFlag = if ($Debug) { "" } else { "--release" }

# Output directory for unified binaries
$OutputDir = Join-Path $PSScriptRoot ".." "target" "agent-binaries"
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$Results = @()

function Build-Agent {
    param(
        [string]$Name,
        [scriptblock]$BuildCmd,
        [string]$BinaryName,
        [string]$SourcePath
    )

    Write-Host "`n=== Building $Name ===" -ForegroundColor Cyan
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    try {
        & $BuildCmd
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed with exit code $LASTEXITCODE"
        }
        $sw.Stop()

        # Copy binary to output directory
        if (Test-Path $SourcePath) {
            Copy-Item -Path $SourcePath -Destination $OutputDir -Force
            $binaryDest = Join-Path $OutputDir $BinaryName
            Write-Host "  Copied: $binaryDest" -ForegroundColor Green
        } else {
            Write-Host "  Warning: Binary not found at $SourcePath" -ForegroundColor Yellow
        }

        $Results += [PSCustomObject]@{
            Agent   = $Name
            Status  = "OK"
            Duration = $sw.Elapsed.ToString("mm\:ss")
        }
    }
    catch {
        $sw.Stop()
        Write-Host "  FAILED: $_" -ForegroundColor Red
        $Results += [PSCustomObject]@{
            Agent   = $Name
            Status  = "FAILED"
            Duration = $sw.Elapsed.ToString("mm\:ss")
        }
    }
}

# ── 1. Claude Code Agent ──
Build-Agent -Name "Claude Code Agent" `
    -BuildCmd { cargo build --package remote-code $ProfileFlag } `
    -BinaryName "remote-code.exe" `
    -SourcePath (Join-Path $PSScriptRoot ".." "target" $Profile "remote-code.exe")

# ── 2. Codex Agent ──
Build-Agent -Name "Codex Agent" `
    -BuildCmd { Push-Location (Join-Path $PSScriptRoot ".." "agents" "codex" "codex-rs"); cargo build --package codex-cli $ProfileFlag; Pop-Location } `
    -BinaryName "codex.exe" `
    -SourcePath (Join-Path $PSScriptRoot ".." "agents" "codex" "codex-rs" "target" $Profile "codex.exe")

# ── 3. Roo-code Agent ──
Build-Agent -Name "Roo-code Agent" `
    -BuildCmd { cargo build --package roo-cli $ProfileFlag } `
    -BinaryName "roo.exe" `
    -SourcePath (Join-Path $PSScriptRoot ".." "target" $Profile "roo.exe")

# ── Summary ──
Write-Host "`n========================================" -ForegroundColor White
Write-Host "  Build Summary (Profile: $Profile)" -ForegroundColor White
Write-Host "========================================" -ForegroundColor White
$Results | Format-Table -AutoSize
Write-Host "Output directory: $OutputDir`n" -ForegroundColor Cyan
