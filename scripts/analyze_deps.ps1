$migratedPkgs = @(
    # Stage 1: leaf crates
    'codex-ansi-escape','codex-async-utils','codex-aws-auth',
    'codex-backend-openapi-models','codex-experimental-api-macros',
    'codex-collaboration-mode-templates','codex-device-key',
    'codex-execpolicy-legacy','codex-file-search','codex-keyring-store',
    'codex-process-hardening','codex-realtime-webrtc',
    'codex-terminal-detection','codex-uds','codex-v8-poc',
    'codex-utils-absolute-path','codex-utils-cache','codex-utils-cargo-bin',
    'codex-utils-elapsed','codex-utils-fuzzy-match','codex-utils-json-to-toml',
    'codex-utils-pty','codex-utils-readiness','codex-utils-rustls-provider',
    'codex-utils-sleep-inhibitor','codex-utils-stream-parser',
    'codex-utils-string','codex-utils-template',
    # Stage 2
    'codex-responses-api-proxy','codex-skills','codex-stdio-to-uds',
    'codex-utils-home-dir','codex-utils-path',
    # Stage 3
    'codex-client','codex-execpolicy','codex-utils-image',
    'codex-install-context','codex-network-proxy',
    # Stage 4
    'codex-protocol',
    # Stage 5
    'codex-agent-identity','codex-sandboxing','codex-shell-command',
    'codex-utils-approval-presets','codex-utils-cli','codex-utils-output-truncation',
    # Stage 6
    'codex-api','codex-app-server-protocol',
    # Stage 7
    'codex-connectors','codex-debug-client','codex-model-provider-info',
    'codex-otel','codex-response-debug-context',
    # Stage 8
    'codex-features',
    # Stage 9
    'codex-config'
)

$migratedDirs = @(
    'ansi-escape','async-utils','aws-auth','codex-backend-openapi-models',
    'codex-experimental-api-macros','collaboration-mode-templates',
    'device-key','execpolicy-legacy','file-search','keyring-store',
    'process-hardening','realtime-webrtc','terminal-detection','uds','v8-poc',
    'responses-api-proxy','skills','stdio-to-uds',
    'codex-client','execpolicy','install-context','network-proxy','protocol',
    'agent-identity','sandboxing','shell-command',
    'codex-api','app-server-protocol',
    'connectors','debug-client','model-provider-info','otel','response-debug-context',
    'features','config'
)

$migratedUtils = @(
    'absolute-path','cache','cargo-bin','elapsed','fuzzy-match','json-to-toml',
    'pty','readiness','rustls-provider','sleep-inhibitor','stream-parser',
    'string','template','home-dir','path-utils','image',
    'approval-presets','cli','output-truncation'
)

$codexRoot = 'agents\codex\codex-rs'
$readyCrates = @()
$blockedCrates = @()

$dirs = Get-ChildItem -Path $codexRoot -Directory
foreach ($crate in $dirs) {
    $tomlPath = Join-Path $crate.FullName 'Cargo.toml'
    $hasToml = Test-Path $tomlPath
    $isMigrated = $migratedDirs -contains $crate.Name
    $isSpecial = $crate.Name -in @('target','.git','utils','app-server')

    if (-not $hasToml -or $isMigrated -or $isSpecial) { continue }

    $content = Get-Content $tomlPath -Raw
    $pkgMatch = [regex]::Match($content, 'name\s*=\s*"([^"]+)"')
    $pkgName = $pkgMatch.Groups[1].Value

    $allCodexRefs = [regex]::Matches($content, 'codex-[\w-]+') | ForEach-Object { $_.Value } | Sort-Object -Unique
    $intDeps = $allCodexRefs | Where-Object { $_ -ne $pkgName }

    if ($intDeps.Count -eq 0) {
        $readyCrates += "$($crate.Name) (no codex deps)"
    } else {
        $unmigrated = $intDeps | Where-Object { $_ -notin $migratedPkgs }
        if ($unmigrated.Count -eq 0) {
            $readyCrates += "$($crate.Name) -> [$($intDeps -join ', ')]"
        } else {
            $blockedCrates += "$($crate.Name) -> missing: [$($unmigrated -join ', ')]"
        }
    }
}

$utilsDirs = Get-ChildItem -Path (Join-Path $codexRoot 'utils') -Directory -ErrorAction SilentlyContinue
foreach ($crate in $utilsDirs) {
    $tomlPath = Join-Path $crate.FullName 'Cargo.toml'
    $hasToml = Test-Path $tomlPath
    $isMigrated = $migratedUtils -contains $crate.Name
    if (-not $hasToml -or $isMigrated) { continue }

    $content = Get-Content $tomlPath -Raw
    $pkgMatch = [regex]::Match($content, 'name\s*=\s*"([^"]+)"')
    $pkgName = $pkgMatch.Groups[1].Value
    $allCodexRefs = [regex]::Matches($content, 'codex-[\w-]+') | ForEach-Object { $_.Value } | Sort-Object -Unique
    $intDeps = $allCodexRefs | Where-Object { $_ -ne $pkgName }

    if ($intDeps.Count -eq 0) {
        $readyCrates += "utils/$($crate.Name) (no codex deps)"
    } else {
        $unmigrated = $intDeps | Where-Object { $_ -notin $migratedPkgs }
        if ($unmigrated.Count -eq 0) {
            $readyCrates += "utils/$($crate.Name) -> [$($intDeps -join ', ')]"
        } else {
            $blockedCrates += "utils/$($crate.Name) -> missing: [$($unmigrated -join ', ')]"
        }
    }
}

Write-Host "=== READY TO MIGRATE ($($readyCrates.Count)) ==="
$readyCrates | ForEach-Object { Write-Host "  $_" }
Write-Host ""
Write-Host "=== BLOCKED ($($blockedCrates.Count)) ==="
$blockedCrates | ForEach-Object { Write-Host "  $_" }
