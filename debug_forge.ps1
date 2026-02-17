$env:RUST_BACKTRACE=1
cargo run -p forge-cli -- start 502f5a32-c83a-435f-aee2-a2c9bc672319 2> start_error.log
if ($LASTEXITCODE -ne 0) {
    Write-Host "Start failed"
}
cargo run -p forge-cli -- ask "Usa git.status para ver el estado del repositorio en C:\Users\javie\dev\forge" > ask_output.txt 2> ask_error.log
if ($LASTEXITCODE -ne 0) {
    Write-Host "Ask failed"
}
