Set-Location 'D:\remote-code-rust\apps\remote-code-gui'
npm run desktop:build *> 'D:\remote-code-rust\.codex-logs\desktop-build-final\desktop-build.log'
$LASTEXITCODE | Out-File -FilePath 'D:\remote-code-rust\.codex-logs\desktop-build-final\exit.txt' -Encoding ascii
