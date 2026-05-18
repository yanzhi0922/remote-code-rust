param(
    [string]$Repo = "yanzhi0922/remote-code-rust",
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\RemoteCode",
    [switch]$Silent,
    [switch]$UseProxy,
    [string]$ProxyUrl = "http://127.0.0.1:7890"
)

$ErrorActionPreference = "Stop"

if ($UseProxy) {
    $env:HTTP_PROXY = $ProxyUrl
    $env:HTTPS_PROXY = $ProxyUrl
}

function Get-ReleaseMetadata {
    if ($Version -eq "latest") {
        $url = "https://api.github.com/repos/$Repo/releases/latest"
    } else {
        $url = "https://api.github.com/repos/$Repo/releases/tags/$Version"
    }
    Invoke-RestMethod -Headers @{ "User-Agent" = "remote-code-installer" } -Uri $url
}

function Select-InstallerAsset($release) {
    $release.assets |
        Where-Object {
            $_.name -match "Remote-Code-windows-x64-setup\.exe$" -or
            $_.name -match "Remote[-_ ]Code.*(x64|x86_64).*setup\.exe$"
        } |
        Select-Object -First 1
}

$release = Get-ReleaseMetadata
$asset = Select-InstallerAsset $release
if (-not $asset) {
    throw "No Windows x64 installer asset found in $Repo release $($release.tag_name)."
}

$downloadDir = Join-Path $env:TEMP "remote-code-install"
New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
$installer = Join-Path $downloadDir $asset.name

Write-Host "Downloading Remote Code $($release.tag_name): $($asset.name)"
Invoke-WebRequest -Headers @{ "User-Agent" = "remote-code-installer" } -Uri $asset.browser_download_url -OutFile $installer

$shaAsset = $release.assets |
    Where-Object { $_.name -eq ($asset.name -replace "\.exe$", ".sha256") } |
    Select-Object -First 1
if ($shaAsset) {
    $shaPath = Join-Path $downloadDir $shaAsset.name
    Invoke-WebRequest -Headers @{ "User-Agent" = "remote-code-installer" } -Uri $shaAsset.browser_download_url -OutFile $shaPath
    $expectedHash = ((Get-Content $shaPath | Select-Object -First 1) -split "\s+")[0]
    $actualHash = (Get-FileHash -Algorithm SHA256 $installer).Hash
    if ($actualHash -ne $expectedHash) {
        throw "Installer checksum mismatch. Expected $expectedHash, got $actualHash."
    }
}

$args = @()
if ($Silent) {
    $args += "/S"
    $args += "/D=$InstallDir"
}

Write-Host "Starting installer..."
$process = Start-Process -FilePath $installer -ArgumentList $args -Wait -PassThru
if ($process.ExitCode -ne 0) {
    throw "Installer failed with exit code $($process.ExitCode)."
}

Write-Host "Remote Code installation completed."
