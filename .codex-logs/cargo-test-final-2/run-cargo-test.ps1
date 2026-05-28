Set-Location 'D:\remote-code-rust'
cargo test --workspace *> 'D:\remote-code-rust\.codex-logs\cargo-test-final-2\cargo-test.log'
$LASTEXITCODE | Out-File -FilePath 'D:\remote-code-rust\.codex-logs\cargo-test-final-2\exit.txt' -Encoding ascii
