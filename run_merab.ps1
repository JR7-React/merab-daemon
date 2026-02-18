# Helper script to run the latest version of Merab
Write-Host "Compiling and running Merab..." -ForegroundColor Cyan
cargo run -q -p merab-cli -- @args
