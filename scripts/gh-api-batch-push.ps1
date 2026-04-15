#!/usr/bin/env pwsh
# gh-api-batch-push.ps1 — 使用 Git Data API 批量推送文件到 main 分支
# 用法: pwsh scripts/gh-api-batch-push.ps1 <commit-message>

param(
    [Parameter(Mandatory=$true)]
    [string]$CommitMessage
)

$repo = "yanzhi0922/remote-code-rust"
$apiBase = "repos/$repo"
$ErrorActionPreference = "Stop"

# Step 1: Get latest commit on main
Write-Host "Fetching latest main commit..."
$latestCommit = (gh api "$apiBase/commits/main" --jq '.sha')
Write-Host "Latest commit: $latestCommit"

# Step 2: Get list of changed files from git
Write-Host "Detecting changed files..."
$gitStatus = git status --porcelain 2>&1
$files = @()

foreach ($line in $gitStatus) {
    $status = $line.Substring(0, 2).Trim()
    $filePath = $line.Substring(3).TrimStart('"').TrimEnd('"')
    
    # Normalize path separators
    $filePath = $filePath.Replace('\', '/')
    
    if ($status -eq 'D' -or $status -eq ' D') {
        # Deleted file - skip for now
        Write-Host "  SKIP (deleted): $filePath"
        continue
    }
    
    if ($status -match '^[MADRC\?\?]+\s') {
        $files += $filePath
        Write-Host "  ADD: $filePath"
    }
}

if ($files.Count -eq 0) {
    Write-Host "No files to push."
    exit 0
}

Write-Host "Total files to push: $($files.Count)"

# Step 3: Create blobs for each file
Write-Host "Creating blobs..."
$blobs = @{}
foreach ($file in $files) {
    $fullPath = (Resolve-Path $file -ErrorAction SilentlyContinue)
    if (-not $fullPath) {
        Write-Host "  SKIP (not found): $file"
        continue
    }
    
    $bytes = [System.IO.File]::ReadAllBytes($fullPath.Path)
    $b64 = [Convert]::ToBase64String($bytes)
    
    # Create blob via API
    $payload = @{ content = $b64; encoding = "base64" } | ConvertTo-Json -Compress
    $payloadFile = [System.IO.Path]::GetTempFileName()
    [System.IO.File]::WriteAllText($payloadFile, $payload)
    
    $blobSha = (gh api "$apiBase/git/blobs" -X POST --input $payloadFile --jq '.sha')
    Remove-Item $payloadFile -ErrorAction SilentlyContinue
    
    $blobs[$file] = $blobSha
    Write-Host "  BLOB: $file -> $blobSha"
}

# Step 4: Create tree
Write-Host "Creating tree..."
$treePayload = @{ base_tree = $latestCommit; tree = @() } | ConvertTo-Json -Depth 5 -Compress

# Build tree items manually since ConvertTo-Json doesn't handle arrays well
$treeItems = @()
foreach ($file in $blobs.Keys) {
    $treeItems += @{ path = $file; mode = "100644"; type = "blob"; sha = $blobs[$file] }
}

$treeJson = @{
    base_tree = $latestCommit
    tree = $treeItems
} | ConvertTo-Json -Depth 5 -Compress

$treeFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($treeFile, $treeJson)

$treeSha = (gh api "$apiBase/git/trees" -X POST --input $treeFile --jq '.sha')
Remove-Item $treeFile -ErrorAction SilentlyContinue
Write-Host "Tree SHA: $treeSha"

# Step 5: Create commit
Write-Host "Creating commit..."
$commitPayload = @{
    message = $CommitMessage
    tree = $treeSha
    parents = @($latestCommit)
} | ConvertTo-Json -Depth 5 -Compress

$commitFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($commitFile, $commitPayload)

$newCommit = (gh api "$apiBase/git/commits" -X POST --input $commitFile --jq '.sha')
Remove-Item $commitFile -ErrorAction SilentlyContinue
Write-Host "New commit: $newCommit"

# Step 6: Update main ref
Write-Host "Updating main ref..."
$refPayload = @{ sha = $newCommit; force = $false } | ConvertTo-Json -Compress
$refFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($refFile, $refPayload)

$result = gh api "$apiBase/git/refs/heads/main" -X PATCH --input $refFile 2>&1
Remove-Item $refFile -ErrorAction SilentlyContinue
Write-Host $result

Write-Host "`nDone! Commit $newCommit pushed to main."
