# Gascii Installer for Windows

$Repo = "eleulleuso/Gascii"
$InstallDir = "$env:LOCALAPPDATA\Gascii"
$BinaryName = "bad_apple.exe"
$Url = "https://github.com/$Repo/releases/latest/download/bad_apple-windows.exe"

Write-Host "🚀 Installing Gascii..." -ForegroundColor Cyan

# Create Install Directory
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

$OutputPath = Join-Path $InstallDir $BinaryName

Write-Host "⬇️  Downloading from: $Url"
try {
    Invoke-WebRequest -Uri $Url -OutFile $OutputPath
} catch {
    Write-Error "❌ Download failed: $_"
    exit 1
}

Write-Host "📦 Installed to $OutputPath"

# Add to PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "🔧 Adding to PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "✅ Added to PATH. Please restart your terminal." -ForegroundColor Yellow
} else {
    Write-Host "✅ Already in PATH."
}

Write-Host "✅ Installation complete! Run 'bad_apple' to start." -ForegroundColor Green
