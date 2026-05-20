param(
    [string]$OutputRoot = ".release-evidence",
    [string]$StressWorkspace = "C:\Users\Yanzh\Desktop\cli-stress-test",
    [switch]$RunBaseGates,
    [switch]$IncludeWorkspaceTests,
    [switch]$IncludeDesktopBundle,
    [switch]$IncludeProviderMatrix,
    [switch]$IncludeMcpMatrix,
    [switch]$IncludeRemoteE2E,
    [switch]$IncludeMobilePwaE2E,
    [switch]$IncludeTransportE2E,
    [switch]$IncludeTailscaleE2E,
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

$Stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$EvidenceRoot = Join-Path $RepoRootPath (Join-Path $OutputRoot $Stamp)
$LogRoot = Join-Path $EvidenceRoot "logs"
New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
New-Item -ItemType Directory -Force -Path $StressWorkspace | Out-Null

$ReportPath = Join-Path $EvidenceRoot "release-acceptance.md"
$Results = New-Object System.Collections.Generic.List[object]

function Add-Result(
    [string]$Area,
    [string]$Item,
    [string]$Status,
    [string]$Evidence,
    [string]$Notes
) {
    $Results.Add([pscustomobject]@{
        Area = $Area
        Item = $Item
        Status = $Status
        Evidence = $Evidence
        Notes = $Notes
    }) | Out-Null
}

function Sanitize-Text([string]$Text) {
    if ($null -eq $Text) {
        return ""
    }
    $patterns = @(
        "sk-[A-Za-z0-9_\-]{12,}",
        "sk-cp-[A-Za-z0-9_\-]{12,}",
        "kV[A-Za-z0-9_\-]{20,}",
        "Bearer\s+[A-Za-z0-9_\-\.]{12,}",
        "x-api-key:\s*\S+"
    )
    $redacted = $Text
    foreach ($pattern in $patterns) {
        $redacted = [regex]::Replace($redacted, $pattern, "<redacted>", "IgnoreCase")
    }
    return $redacted
}

function Run-LoggedStep([string]$Area, [string]$Item, [scriptblock]$Command) {
    $safeName = ($Area + "-" + $Item).ToLowerInvariant() -replace "[^a-z0-9]+", "-"
    $logPath = Join-Path $LogRoot "$safeName.log"
    Write-Host ""
    Write-Host "=== $Area / $Item ==="
    try {
        $global:LASTEXITCODE = 0
        $output = & $Command 2>&1 | Out-String
        if ($global:LASTEXITCODE -ne 0) {
            throw "command exited with code $global:LASTEXITCODE`n$output"
        }
        $sanitized = Sanitize-Text $output
        Set-Content -Path $logPath -Value $sanitized -Encoding UTF8
        Add-Result $Area $Item "PASS" $logPath ""
    }
    catch {
        $message = Sanitize-Text ($_.Exception.Message)
        Set-Content -Path $logPath -Value $message -Encoding UTF8
        Add-Result $Area $Item "FAIL" $logPath $message
    }
}

function Env-Present([string[]]$Names) {
    foreach ($name in $Names) {
        if (-not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
            return $true
        }
    }
    return $false
}

function RemoteCodeExe {
    $release = Join-Path $RepoRootPath "target\release\remote-code.exe"
    if (Test-Path $release) {
        return $release
    }
    return Join-Path $RepoRootPath "target\debug\remote-code.exe"
}

function Ensure-RemoteCodeExe {
    $exe = RemoteCodeExe
    if (-not (Test-Path $exe)) {
        cargo build -p remote-code --bin remote-code -j 1
    }
    return (RemoteCodeExe)
}

function Invoke-ProviderSmoke(
    [string]$Provider,
    [string]$BaseUrl,
    [string]$Model,
    [string]$Protocol,
    [string[]]$RequiredEnv
) {
    $exe = Ensure-RemoteCodeExe
    $profile = Join-Path $EvidenceRoot ("profile-" + $Provider)
    New-Item -ItemType Directory -Force -Path $profile | Out-Null
    & $exe --print --cwd $StressWorkspace --profile-dir $profile --provider $Provider --base-url $BaseUrl --model $Model --protocol $Protocol --permission-mode bypassPermissions --max-turns 3 "Reply exactly: RC_PROVIDER_SMOKE_OK"
}

function Write-McpAcceptanceConfig {
    $configPath = Join-Path $EvidenceRoot "mcp.acceptance.json"
    $servers = [ordered]@{
        context7 = @{
            command = "npx"
            args = @("-y", "@upstash/context7-mcp")
            env = @{ DEFAULT_MINIMUM_TOKENS = "" }
        }
        sequentialthinking = @{
            command = "npx"
            args = @("-y", "@modelcontextprotocol/server-sequential-thinking")
        }
        memory = @{
            command = "npx"
            args = @("-y", "@modelcontextprotocol/server-memory")
        }
        puppeteer = @{
            command = "npx"
            args = @("-y", "@modelcontextprotocol/server-puppeteer")
        }
    }
    if (Env-Present @("MINIMAX_API_KEY", "MINIMAX_TOKEN_PLAN_API_KEY")) {
        $minimaxKeyEnv = if (-not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("MINIMAX_API_KEY"))) {
            '${MINIMAX_API_KEY}'
        } else {
            '${MINIMAX_TOKEN_PLAN_API_KEY}'
        }
        $minimaxHostEnv = if (-not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable("MINIMAX_API_HOST"))) {
            '${MINIMAX_API_HOST}'
        } else {
            'https://api.minimaxi.com'
        }
        $servers.MiniMax = @{
            command = "uvx"
            args = @("minimax-coding-plan-mcp", "-y")
            env = @{
                MINIMAX_API_KEY = $minimaxKeyEnv
                MINIMAX_API_HOST = $minimaxHostEnv
            }
        }
    }
    $payload = @{ mcpServers = $servers } | ConvertTo-Json -Depth 8
    [System.IO.File]::WriteAllText(
        $configPath,
        $payload,
        [System.Text.UTF8Encoding]::new($false)
    )
    return $configPath
}

function Invoke-McpList([string]$Server, [string]$ConfigPath) {
    $exe = Ensure-RemoteCodeExe
    & $exe mcp list --connect --json --server $Server --config $ConfigPath
}

function Invoke-McpToolCall([string]$Server, [string]$ConfigPath) {
    $exe = Ensure-RemoteCodeExe
    switch ($Server) {
        "context7" {
            & $exe mcp call --json --server $Server --tool resolve-library-id --arg "libraryName=tokio" --arg "query=Rust async runtime documentation" --config $ConfigPath
        }
        "sequentialthinking" {
            & $exe mcp call --json --server $Server --tool sequentialthinking --arg "thought=acceptance smoke test" --arg "nextThoughtNeeded=false" --arg "thoughtNumber=1" --arg "totalThoughts=1" --config $ConfigPath
        }
        "memory" {
            & $exe mcp call --json --server $Server --tool read_graph --config $ConfigPath
        }
        "puppeteer" {
            & $exe mcp call --json --server $Server --tool puppeteer_navigate --arg "url=about:blank" --config $ConfigPath
        }
        "MiniMax" {
            & $exe mcp list --connect --json --server $Server --config $ConfigPath
        }
    }
}

if ($RunBaseGates) {
    Run-LoggedStep "14.1" "base-gates" {
        $args = @("-ExecutionPolicy", "Bypass", "-File", (Join-Path $RepoRootPath "scripts\verify-release.ps1"), "-IncludeAudit", "-IncludeGitleaks")
        if ($IncludeWorkspaceTests) { $args += "-IncludeWorkspaceTests" }
        if ($IncludeDesktopBundle) { $args += "-IncludeDesktopBundle" }
        if ($UseProxy) { $args += @("-UseProxy", "-ProxyUrl", $ProxyUrl) }
        powershell @args
    }
} else {
    Add-Result "14.1" "base-gates" "SKIP" "" "pass -RunBaseGates to execute local release gates"
}

if ($IncludeProviderMatrix) {
    $providerMatrix = @(
        @{ Name = "minimax-token-plan"; BaseUrl = "https://api.minimaxi.com/anthropic"; Model = "minimax-m2.7"; Protocol = "anthropic"; Env = @("MINIMAX_TOKEN_PLAN_API_KEY", "MINIMAX_API_KEY") },
        @{ Name = "kuaikat-coding"; BaseUrl = "https://wanqing.streamlakeapi.com/api/gateway/coding/kat-coder-pro-v2/claude-code-proxy"; Model = "kat-coder-pro-v2"; Protocol = "anthropic"; Env = @("KUAIKAT_CODING_PLAN_API_KEY", "KUAIKAT_API_KEY") },
        @{ Name = "deepseek-anthropic"; BaseUrl = "https://api.deepseek.com/anthropic"; Model = "deepseek-v4-flash"; Protocol = "anthropic"; Env = @("DEEPSEEK_API_KEY", "DEEPSEEK_CODING_PLAN_API_KEY") }
    )
    foreach ($provider in $providerMatrix) {
        if (-not (Env-Present $provider.Env)) {
            Add-Result "Provider" $provider.Name "SKIP" "" ("missing env: " + ($provider.Env -join " or "))
            continue
        }
        Run-LoggedStep "Provider" $provider.Name {
            Invoke-ProviderSmoke $provider.Name $provider.BaseUrl $provider.Model $provider.Protocol $provider.Env
        }
    }
} else {
    Add-Result "Provider" "matrix" "SKIP" "" "pass -IncludeProviderMatrix with provider keys in environment"
}

if ($IncludeMcpMatrix) {
    $mcpConfig = Write-McpAcceptanceConfig
    foreach ($server in @("context7", "sequentialthinking", "memory", "puppeteer", "MiniMax")) {
        if ($server -eq "MiniMax" -and -not (Env-Present @("MINIMAX_API_KEY", "MINIMAX_TOKEN_PLAN_API_KEY"))) {
            Add-Result "MCP" "MiniMax" "SKIP" "" "missing MINIMAX_API_KEY or MINIMAX_TOKEN_PLAN_API_KEY"
            continue
        }
        Run-LoggedStep "MCP" "$server-discover" { Invoke-McpList $server $mcpConfig }
        Run-LoggedStep "MCP" "$server-call" { Invoke-McpToolCall $server $mcpConfig }
    }
} else {
    Add-Result "MCP" "matrix" "SKIP" "" "pass -IncludeMcpMatrix to start and call MCP servers"
}

if ($IncludeRemoteE2E) {
    Add-Result "Remote E2E" "relay-runner-control-plane" "MANUAL" "" "requires a deployed relay host and trusted runner endpoint"
} else {
    Add-Result "Remote E2E" "relay-runner-control-plane" "SKIP" "" "pass -IncludeRemoteE2E after provisioning relay and runner"
}

if ($IncludeMobilePwaE2E) {
    Add-Result "Mobile/PWA" "pairing-prompt-approval-artifact" "MANUAL" "" "requires real mobile browser/device install and screenshots"
} else {
    Add-Result "Mobile/PWA" "pairing-prompt-approval-artifact" "SKIP" "" "pass -IncludeMobilePwaE2E after preparing real devices"
}

if ($IncludeTransportE2E) {
    Add-Result "Transport" "relay-direct-outbound-quic" "MANUAL" "" "requires controlled network endpoints, TLS/cert fingerprint evidence, and QUIC-enabled relay"
} else {
    Add-Result "Transport" "relay-direct-outbound-quic" "SKIP" "" "pass -IncludeTransportE2E after provisioning transport testbed"
}

if ($IncludeTailscaleE2E) {
    Add-Result "Tailscale" "tailnet-direct-fallback" "MANUAL" "" "requires a tailnet with ACL/device-trust evidence"
} else {
    Add-Result "Tailscale" "tailnet-direct-fallback" "SKIP" "" "pass -IncludeTailscaleE2E in a prepared tailnet"
}

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Release Acceptance Evidence")
$lines.Add("")
$lines.Add("- Date: $(Get-Date -Format o)")
$lines.Add("- Repository: $RepoRootPath")
$lines.Add("- Stress workspace: $StressWorkspace")
$lines.Add("- Output root: $EvidenceRoot")
$lines.Add("- Secret policy: logs are sanitized; provider keys must be supplied only through environment variables.")
$lines.Add("")
$lines.Add("| Area | Item | Status | Evidence | Notes |")
$lines.Add("| --- | --- | --- | --- | --- |")
foreach ($result in $Results) {
    $evidence = if ([string]::IsNullOrWhiteSpace($result.Evidence)) { "" } else { $result.Evidence.Replace($RepoRootPath, ".") }
    $notes = ($result.Notes -replace "\|", "/").Replace("`r", " ").Replace("`n", " ")
    $lines.Add("| $($result.Area) | $($result.Item) | $($result.Status) | $evidence | $notes |")
}
$lines.Add("")
$lines.Add("## Manual Sign-Off")
$lines.Add("")
$lines.Add("- Release engineer:")
$lines.Add("- Relay host:")
$lines.Add("- Desktop installer hash:")
$lines.Add("- Mobile/PWA device matrix:")
$lines.Add("- QUIC/Tailscale environment:")
$lines.Add("- Provider/MCP matrix owner:")

Set-Content -Path $ReportPath -Value $lines -Encoding UTF8
Write-Host ""
Write-Host "Acceptance evidence written to $ReportPath"
