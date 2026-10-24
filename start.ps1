# RARA Enterprise ERP — Development Startup Script
# Run this from the d:\rara directory in PowerShell

Write-Host "🚀 Starting RARA Enterprise ERP Development Environment" -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor DarkYellow

# Start Frontend in a new window
Write-Host "`n📱 Starting Frontend (SvelteKit on port 5173)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit -Command `"cd d:\rara\frontend; npm run dev`"" -WindowStyle Normal

Start-Sleep 2

# Try to start Backend
$cargoPath = "$env:USERPROFILE\.cargo\bin\cargo.exe"
$gccPath = "d:\rara\w64devkit\bin"

if (Test-Path $cargoPath) {
    Write-Host "🦀 Starting Backend (Rust Axum on port 8080)..." -ForegroundColor Green
    Start-Process powershell -ArgumentList "-NoExit -Command `"`$env:PATH = '$gccPath;$env:USERPROFILE\.cargo\bin;' + `$env:PATH; cd d:\rara\backend; cargo run --bin server`"" -WindowStyle Normal
} else {
    Write-Host "⚠️  Rust not found in $env:USERPROFILE\.cargo\bin" -ForegroundColor Red
}

Write-Host "`n✅ Services starting:" -ForegroundColor Green
Write-Host "   Frontend: http://localhost:5173" -ForegroundColor White
Write-Host "   Backend:  http://localhost:8080" -ForegroundColor White
Write-Host "   Health:   http://localhost:8080/health" -ForegroundColor White
Write-Host "`n📖 Default login: admin@raraenterprise.com" -ForegroundColor Yellow
Write-Host "⚠️  Set admin password via backend before using!" -ForegroundColor Red
