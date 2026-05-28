Set-Location 'D:\remote-code-rust'
cargo test --workspace *> 'D:\remote-code-rust\.codex-logs\cargo-test-final\cargo-test.log'
$LASTEXITCODE | Out-File -FilePath 'D:\remote-code-rust\.codex-logs\cargo-test-final\exit.txt' -Encoding ascii
