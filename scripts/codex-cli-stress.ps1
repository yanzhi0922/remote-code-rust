#Requires -Version 7.0
<#
.SYNOPSIS
    Smoke/stress checks for Codex CLI isolation and basic MCP commands.

.DESCRIPTION
    Uses isolated CODEX_HOME directories under a disposable stress root to verify
    that MCP configuration written in one home does not leak into another. The
    script defaults to the repository build output and only runs local CLI
    commands that do not call a model.

.EXAMPLE
    ./scripts/codex-cli-stress.ps1
    ./scripts/codex-cli-stress.ps1 -CodexExe C:\path\to\codex.exe
    ./scripts/codex-cli-stress.ps1 -StressRoot C:\tmp\cli-stress-test
#>

[CmdletBinding()]
param(
    [string]$CodexExe,
    [string]$StressRoot = (Join-Path ([System.IO.Path]::GetTempPath()) "remote-code-codex-cli-stress")
)

$ErrorActionPreference = "Stop"

Set-StrictMode -Version Latest

$script:Results = New-Object System.Collections.Generic.List[object]

function Resolve-CodexExe {
    param([string]$RequestedPath)

    if ($RequestedPath) {
        $resolved = Resolve-Path -LiteralPath $RequestedPath -ErrorAction SilentlyContinue
        if (-not $resolved) {
            throw "codex.exe not found at requested path: $RequestedPath"
        }
        return $resolved.Path
    }

    $candidates = @(
        (Join-Path $PSScriptRoot "..\target\release\codex.exe"),
        (Join-Path $PSScriptRoot "..\target\debug\codex.exe")
    )

    foreach ($candidate in $candidates) {
        $resolvedCandidate = Resolve-Path -LiteralPath $candidate -ErrorAction SilentlyContinue
        if ($resolvedCandidate) {
            return $resolvedCandidate.Path
        }
    }

    throw "repository codex.exe not found under target\release or target\debug. Build it first or pass -CodexExe <path>."
}

function Reset-StressDirectory {
    param([string]$Root)

    $rootPath = [System.IO.Path]::GetFullPath($Root)
    New-Item -ItemType Directory -Path $rootPath -Force | Out-Null

    foreach ($child in @("codex-home-a", "codex-home-b", "work-a", "work-b")) {
        $path = [System.IO.Path]::GetFullPath((Join-Path $rootPath $child))
        if (-not $path.StartsWith($rootPath, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to touch path outside stress root: $path"
        }
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
        New-Item -ItemType Directory -Path $path -Force | Out-Null
    }
}

function Add-Result {
    param(
        [string]$Name,
        [bool]$Passed,
        [string]$Detail = ""
    )

    $script:Results.Add([PSCustomObject]@{
        Name = $Name
        Passed = $Passed
        Detail = $Detail
    }) | Out-Null

    $status = if ($Passed) { "PASS" } else { "FAIL" }
    $color = if ($Passed) { "Green" } else { "Red" }
    if ($Detail) {
        Write-Host ("[{0}] {1} - {2}" -f $status, $Name, $Detail) -ForegroundColor $color
    } else {
        Write-Host ("[{0}] {1}" -f $status, $Name) -ForegroundColor $color
    }
}

function Invoke-Codex {
    param(
        [string]$CodexHome,
        [string]$WorkDir,
        [string[]]$Arguments
    )

    $oldCodexHome = $env:CODEX_HOME
    try {
        $env:CODEX_HOME = $CodexHome
        Push-Location -LiteralPath $WorkDir
        try {
            $output = & $script:CodexExeResolved @Arguments 2>&1
            $exitCode = $LASTEXITCODE
        } finally {
            Pop-Location
        }
    } finally {
        $env:CODEX_HOME = $oldCodexHome
    }

    return [PSCustomObject]@{
        ExitCode = $exitCode
        Output = ($output | Out-String).Trim()
    }
}

function Assert-Command {
    param(
        [string]$Name,
        [string]$CodexHome,
        [string]$WorkDir,
        [string[]]$Arguments,
        [scriptblock]$Validate
    )

    $result = Invoke-Codex -CodexHome $CodexHome -WorkDir $WorkDir -Arguments $Arguments
    $ok = $result.ExitCode -eq 0
    $detail = "exit=$($result.ExitCode)"

    if ($ok -and $Validate) {
        try {
            $validationDetail = & $Validate $result
            if ($validationDetail) {
                $detail = $validationDetail
            }
        } catch {
            $ok = $false
            $detail = $_.Exception.Message
        }
    } elseif (-not $ok -and $result.Output) {
        $detail = "exit=$($result.ExitCode); $($result.Output)"
    }

    Add-Result -Name $Name -Passed $ok -Detail $detail
    return $result
}

function Get-McpListJson {
    param(
        [string]$CodexHome,
        [string]$WorkDir
    )

    $result = Invoke-Codex -CodexHome $CodexHome -WorkDir $WorkDir -Arguments @("mcp", "list", "--json")
    if ($result.ExitCode -ne 0) {
        throw "mcp list --json failed with exit=$($result.ExitCode): $($result.Output)"
    }
    return $result.Output | ConvertFrom-Json
}

function Get-McpServerJson {
    param(
        [string]$CodexHome,
        [string]$WorkDir,
        [string]$Name
    )

    $result = Invoke-Codex -CodexHome $CodexHome -WorkDir $WorkDir -Arguments @("mcp", "get", $Name, "--json")
    if ($result.ExitCode -ne 0) {
        throw "mcp get $Name --json failed with exit=$($result.ExitCode): $($result.Output)"
    }
    return $result.Output | ConvertFrom-Json
}

function Test-ToolPresence {
    param([string]$ToolName)

    $cmd = Get-Command $ToolName -ErrorAction SilentlyContinue
    if ($cmd) {
        Add-Result -Name "tool available: $ToolName" -Passed $true -Detail $cmd.Source
    } else {
        Add-Result -Name "tool available: $ToolName" -Passed $false -Detail "not found on PATH"
    }
}

$script:CodexExeResolved = Resolve-CodexExe -RequestedPath $CodexExe
$stressRootFull = [System.IO.Path]::GetFullPath($StressRoot)

Write-Host "Codex CLI stress root: $stressRootFull" -ForegroundColor Cyan
Write-Host "Codex executable: $script:CodexExeResolved" -ForegroundColor Cyan
Write-Host "Model calls: disabled by design; only local help/config/debug commands are executed." -ForegroundColor Cyan

Reset-StressDirectory -Root $stressRootFull

$homeA = Join-Path $stressRootFull "codex-home-a"
$homeB = Join-Path $stressRootFull "codex-home-b"
$workA = Join-Path $stressRootFull "work-a"
$workB = Join-Path $stressRootFull "work-b"

Assert-Command -Name "codex --version" -CodexHome $homeA -WorkDir $workA -Arguments @("--version") -Validate {
    param($result)
    if ($result.Output -notmatch "codex") {
        throw "version output did not contain 'codex': $($result.Output)"
    }
    return $result.Output
} | Out-Null

Assert-Command -Name "codex exec --help" -CodexHome $homeA -WorkDir $workA -Arguments @("exec", "--help") -Validate {
    param($result)
    if ($result.Output -notmatch "codex exec") {
        throw "help output did not contain 'codex exec'"
    }
    return "help text returned"
} | Out-Null

Assert-Command -Name "codex debug clear-memories" -CodexHome $homeA -WorkDir $workA -Arguments @("debug", "clear-memories") -Validate {
    param($result)
    if ($result.Output -and $result.Output -notmatch "Cleared memory") {
        throw "unexpected clear-memories output: $($result.Output)"
    }
    return "memory state cleared or already empty"
} | Out-Null

Assert-Command -Name "mcp add in CODEX_HOME A" -CodexHome $homeA -WorkDir $workA -Arguments @("mcp", "add", "stress-a", "--", "cmd", "/c", "echo", "stress-a") -Validate {
    param($result)
    if ($result.Output -notmatch "stress-a") {
        throw "add output did not mention stress-a: $($result.Output)"
    }
    return "stress-a added"
} | Out-Null

Assert-Command -Name "mcp add in CODEX_HOME B" -CodexHome $homeB -WorkDir $workB -Arguments @("mcp", "add", "stress-b", "--", "cmd", "/c", "echo", "stress-b") -Validate {
    param($result)
    if ($result.Output -notmatch "stress-b") {
        throw "add output did not mention stress-b: $($result.Output)"
    }
    return "stress-b added"
} | Out-Null

try {
    $listA = @(Get-McpListJson -CodexHome $homeA -WorkDir $workA)
    $namesA = @($listA | ForEach-Object { $_.name })
    $aIsolated = ($namesA -contains "stress-a") -and ($namesA -notcontains "stress-b")
    Add-Result -Name "CODEX_HOME A mcp list isolation" -Passed $aIsolated -Detail ("servers=[{0}]" -f ($namesA -join ", "))
} catch {
    Add-Result -Name "CODEX_HOME A mcp list isolation" -Passed $false -Detail $_.Exception.Message
}

try {
    $listB = @(Get-McpListJson -CodexHome $homeB -WorkDir $workB)
    $namesB = @($listB | ForEach-Object { $_.name })
    $bIsolated = ($namesB -contains "stress-b") -and ($namesB -notcontains "stress-a")
    Add-Result -Name "CODEX_HOME B mcp list isolation" -Passed $bIsolated -Detail ("servers=[{0}]" -f ($namesB -join ", "))
} catch {
    Add-Result -Name "CODEX_HOME B mcp list isolation" -Passed $false -Detail $_.Exception.Message
}

try {
    $serverA = Get-McpServerJson -CodexHome $homeA -WorkDir $workA -Name "stress-a"
    $ok = $serverA.name -eq "stress-a" -and $serverA.transport.type -eq "stdio" -and $serverA.transport.command -eq "cmd"
    Add-Result -Name "CODEX_HOME A mcp get" -Passed $ok -Detail ("name={0}; command={1}" -f $serverA.name, $serverA.transport.command)
} catch {
    Add-Result -Name "CODEX_HOME A mcp get" -Passed $false -Detail $_.Exception.Message
}

try {
    $missingInB = Invoke-Codex -CodexHome $homeB -WorkDir $workB -Arguments @("mcp", "get", "stress-a", "--json")
    $ok = $missingInB.ExitCode -ne 0 -and $missingInB.Output -match "No MCP server named"
    Add-Result -Name "CODEX_HOME B cannot get A server" -Passed $ok -Detail ("exit={0}" -f $missingInB.ExitCode)
} catch {
    Add-Result -Name "CODEX_HOME B cannot get A server" -Passed $false -Detail $_.Exception.Message
}

try {
    $configA = Join-Path $homeA "config.toml"
    $configB = Join-Path $homeB "config.toml"
    $configTextA = if (Test-Path -LiteralPath $configA) { Get-Content -LiteralPath $configA -Raw } else { "" }
    $configTextB = if (Test-Path -LiteralPath $configB) { Get-Content -LiteralPath $configB -Raw } else { "" }
    $ok = $configTextA.Contains("stress-a") -and -not $configTextA.Contains("stress-b") -and $configTextB.Contains("stress-b") -and -not $configTextB.Contains("stress-a")
    Add-Result -Name "config.toml contents are isolated" -Passed $ok -Detail "checked both CODEX_HOME config files"
} catch {
    Add-Result -Name "config.toml contents are isolated" -Passed $false -Detail $_.Exception.Message
}

Test-ToolPresence -ToolName "npx"
Test-ToolPresence -ToolName "uvx"

$passed = @($script:Results | Where-Object { $_.Passed }).Count
$failed = @($script:Results | Where-Object { -not $_.Passed }).Count

Write-Host ""
Write-Host ("Summary: {0} PASS, {1} FAIL" -f $passed, $failed) -ForegroundColor $(if ($failed -eq 0) { "Green" } else { "Red" })

Write-Host "Stress directories remain under the configured root for inspection; reruns reset only the test subdirectories." -ForegroundColor Yellow

if ($failed -gt 0) {
    exit 1
}
