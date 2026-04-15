#!/usr/bin/env pwsh
# gh-api-push.ps1 — 使用 gh api Contents API 推送文件到 main 分支
# 用法: pwsh scripts/gh-api-push.ps1 <file-path> <commit-message>

param(
    [Parameter(Mandatory=$true)]
    [string]$FilePath,
    
    [Parameter(Mandatory=$true)]
    [string]$CommitMessage
)

$repo = "yanzhi0922/remote-code-rust"
$apiBase = "repos/$repo/contents"
$apiPath = "$apiBase/$FilePath"

# Step 1: Get current SHA
Write-Host "Fetching SHA for $FilePath..."
$sha = (gh api $apiPath --jq '.sha')
if (-not $sha) {
    Write-Host "File not found on remote, will create new."
    $sha = ""
}

# Step 2: Base64 encode
$fullPath = (Resolve-Path $FilePath).Path
$bytes = [System.IO.File]::ReadAllBytes($fullPath)
$b64 = [Convert]::ToBase64String($bytes)
Write-Host "File size: $($bytes.Length) bytes, Base64 length: $($b64.Length)"

# Step 3: Write base64 to temp file (avoid command line length limit)
$tempFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($tempFile, $b64)
Write-Host "Temp file: $tempFile"

# Step 4: Build JSON payload
if ($sha) {
    $payload = @{
        message = $CommitMessage
        content = $b64
        sha = $sha
    } | ConvertTo-Json -Compress
} else {
    $payload = @{
        message = $CommitMessage
        content = $b64
    } | ConvertTo-Json -Compress
}

# Step 5: Write payload to temp file and use --input
$payloadFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($payloadFile, $payload)

# Step 6: Push via API
Write-Host "Pushing to main via gh api..."
if ($sha) {
    $result = gh api $apiPath -X PUT --input $payloadFile 2>&1
} else {
    $result = gh api $apiPath -X PUT --input $payloadFile 2>&1
}

Write-Host $result

# Cleanup
Remove-Item $tempFile -ErrorAction SilentlyContinue
Remove-Item $payloadFile -ErrorAction SilentlyContinue

Write-Host "Done!"
