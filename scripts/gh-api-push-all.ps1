#!/usr/bin/env pwsh
# gh-api-push-all.ps1 — Push all changed/new files to GitHub main via Contents API
param([string]$Message = "update: push files via gh api")

$repo = "yanzhi0922/remote-code-rust"
$apiBase = "repos/$repo/contents"
$ErrorActionPreference = "Continue"

# Get all changed and untracked files
$allFiles = @()
git diff --name-only | ForEach-Object { $allFiles += $_ }
git ls-files --others --exclude-standard | ForEach-Object { $allFiles += $_ }

# Remove duplicates
$allFiles = $allFiles | Sort-Object -Unique

Write-Host "Files to push: $($allFiles.Count)"
foreach ($f in $allFiles) {
    Write-Host "  $f"
}

foreach ($filePath in $allFiles) {
    $normalizedPath = $filePath.Replace('\', '/')
    Write-Host "`nProcessing: $normalizedPath"
    
    # Read and encode file
    $fullPath = (Resolve-Path $filePath -ErrorAction SilentlyContinue)
    if (-not $fullPath) {
        Write-Host "  SKIP: file not found"
        continue
    }
    
    $bytes = [System.IO.File]::ReadAllBytes($fullPath.Path)
    $b64 = [Convert]::ToBase64String($bytes)
    Write-Host "  Size: $($bytes.Length) bytes"
    
    # Check if file exists on remote (get SHA)
    $sha = $null
    try {
        $sha = (gh api "$apiBase/$normalizedPath" --jq '.sha' 2>$null)
    } catch {}
    
    # Build payload
    $payload = @{
        message = "$Message"
        content = $b64
    }
    if ($sha) {
        $payload.sha = $sha
        Write-Host "  Mode: UPDATE (sha=$sha)"
    } else {
        Write-Host "  Mode: CREATE"
    }
    
    $json = $payload | ConvertTo-Json -Compress
    $tempFile = [System.IO.Path]::GetTempFileName()
    [System.IO.File]::WriteAllText($tempFile, $json)
    
    # Push
    $result = gh api "$apiBase/$normalizedPath" -X PUT --input $tempFile 2>&1
    $commitSha = ($result | ConvertFrom-Json).commit.sha
    Write-Host "  Commit: $commitSha"
    
    Remove-Item $tempFile -ErrorAction SilentlyContinue
}

Write-Host "`nAll files pushed!"
