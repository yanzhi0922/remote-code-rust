param(
    [string]$RooPath,
    [switch]$Debug,
    [switch]$BuildIfMissing,
    [int]$Iterations = 3
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$Profile = if ($Debug) { "debug" } else { "release" }
$ProfileFlag = if ($Debug) { "" } else { "--release" }

if ([string]::IsNullOrWhiteSpace($RooPath)) {
    $RooPath = Join-Path $RepoRoot "target\$Profile\roo.exe"
}

function Run-Step([string]$Name, [scriptblock]$Command) {
    Write-Host ""
    Write-Host "=== $Name ==="
    & $Command
}

function Invoke-Roo([string[]]$Arguments) {
    $output = & $RooPath @Arguments 2>&1
    [PSCustomObject]@{
        ExitCode = $LASTEXITCODE
        Output   = ($output | Out-String)
    }
}

function Assert-Success($Result, [string]$Name) {
    if ($Result.ExitCode -ne 0) {
        throw "$Name failed with exit code $($Result.ExitCode):`n$($Result.Output)"
    }
}

function Assert-Contains($Result, [string]$Expected, [string]$Name) {
    if ($Result.Output -notlike "*$Expected*") {
        throw "$Name did not contain expected text '$Expected'. Output:`n$($Result.Output)"
    }
}

if (-not (Test-Path $RooPath)) {
    if ($BuildIfMissing) {
        Run-Step "Build Roo CLI" {
            cargo build -p roo-cli --bin roo $ProfileFlag
        }
    } else {
        throw "Roo binary not found at $RooPath. Run scripts\build-agents.ps1 or pass -BuildIfMissing."
    }
}

$RooPath = (Resolve-Path $RooPath).Path
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("roo-cli-stress-" + [System.Guid]::NewGuid().ToString("N"))
$ResolvedTempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$ResolvedTempRoot = [System.IO.Path]::GetFullPath($TempRoot)
if (-not $ResolvedTempRoot.StartsWith($ResolvedTempBase, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use temp root outside system temp directory: $ResolvedTempRoot"
}
$HomeDir = Join-Path $TempRoot "home"
$WorkspaceDir = Join-Path $TempRoot "workspace"
$ConfigPath = Join-Path $TempRoot "roo-config.json"
New-Item -ItemType Directory -Path $HomeDir, $WorkspaceDir -Force | Out-Null
Set-Content -Path $ConfigPath -Value '{"provider":"fake-ai","fake_response":"scripted fake reply"}' -Encoding UTF8

$previousUserProfile = $env:USERPROFILE
$previousHome = $env:HOME
$previousAppData = $env:APPDATA
$previousLocalAppData = $env:LOCALAPPDATA
$previousXdgConfigHome = $env:XDG_CONFIG_HOME

try {
    $env:USERPROFILE = $HomeDir
    $env:HOME = $HomeDir
    $env:APPDATA = Join-Path $HomeDir "AppData\Roaming"
    $env:LOCALAPPDATA = Join-Path $HomeDir "AppData\Local"
    $env:XDG_CONFIG_HOME = Join-Path $HomeDir ".config"

    Run-Step "roo.exe --help" {
        $result = Invoke-Roo @("--help")
        Assert-Success $result "roo.exe --help"
        Assert-Contains $result "Roo Code CLI" "roo.exe --help"
    }

    Run-Step "Roo fake provider config isolation" {
        $result = Invoke-Roo @(
            "--config", $ConfigPath,
            "--working-dir", $WorkspaceDir,
            "--message", "hello"
        )
        if ($result.ExitCode -ne 0 -and $result.Output -like "*Unsupported provider*fake-ai*") {
            Write-Warning "Skipping fake provider smoke: current Roo CLI does not support fake-ai."
            return
        }
        Assert-Success $result "Roo fake provider config smoke"
        Assert-Contains $result "scripted fake reply" "Roo fake provider config smoke"
    }

    Run-Step "Roo fake provider stress" {
        for ($i = 1; $i -le $Iterations; $i++) {
            $result = Invoke-Roo @(
                "--provider", "fake-ai",
                "--working-dir", $WorkspaceDir,
                "--message", "stress $i"
            )
            if ($result.ExitCode -ne 0 -and $result.Output -like "*Unsupported provider*fake-ai*") {
                Write-Warning "Skipping fake provider stress: current Roo CLI does not support fake-ai."
                return
            }
            Assert-Success $result "Roo fake provider stress iteration $i"
            Assert-Contains $result "Hello from FakeAI" "Roo fake provider stress iteration $i"
        }
    }
}
finally {
    if ($null -eq $previousUserProfile) { Remove-Item Env:\USERPROFILE -ErrorAction SilentlyContinue } else { $env:USERPROFILE = $previousUserProfile }
    if ($null -eq $previousHome) { Remove-Item Env:\HOME -ErrorAction SilentlyContinue } else { $env:HOME = $previousHome }
    if ($null -eq $previousAppData) { Remove-Item Env:\APPDATA -ErrorAction SilentlyContinue } else { $env:APPDATA = $previousAppData }
    if ($null -eq $previousLocalAppData) { Remove-Item Env:\LOCALAPPDATA -ErrorAction SilentlyContinue } else { $env:LOCALAPPDATA = $previousLocalAppData }
    if ($null -eq $previousXdgConfigHome) { Remove-Item Env:\XDG_CONFIG_HOME -ErrorAction SilentlyContinue } else { $env:XDG_CONFIG_HOME = $previousXdgConfigHome }
    $cleanupRoot = [System.IO.Path]::GetFullPath($TempRoot)
    $cleanupName = Split-Path -Path $cleanupRoot -Leaf
    if ($cleanupRoot.StartsWith($ResolvedTempBase, [System.StringComparison]::OrdinalIgnoreCase) -and $cleanupName.StartsWith("roo-cli-stress-", [System.StringComparison]::Ordinal)) {
        Remove-Item -LiteralPath $cleanupRoot -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        Write-Warning "Skipping unsafe cleanup path: $cleanupRoot"
    }
}

Write-Host ""
Write-Host "Roo CLI smoke/stress complete."
