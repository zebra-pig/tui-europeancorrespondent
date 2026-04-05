# Install The European Correspondent TUI permanently (Windows)
# Usage: irm https://raw.githubusercontent.com/zebra-pig/tui-europeancorrespondent/main/install.ps1 | iex

$repo = "zebra-pig/tui-europeancorrespondent"
$binary = "tui-europeancorrespondent"

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") { "aarch64" } else { "x86_64" }
$asset = "$binary-$arch-pc-windows-msvc.exe"
$url = "https://github.com/$repo/releases/latest/download/$asset"

$installDir = Join-Path $env:LOCALAPPDATA "european-correspondent"
$binPath = Join-Path $installDir "$binary.exe"

New-Item -ItemType Directory -Force -Path $installDir | Out-Null

Write-Host "The European Correspondent - Terminal Edition"
Write-Host "=============================================="
Write-Host ""
Write-Host "Installing to: $binPath"
Write-Host ""

Invoke-WebRequest -Uri $url -OutFile $binPath -UseBasicParsing

Write-Host ""
Write-Host "Installed successfully!"
Write-Host ""
Write-Host "Run with: $binPath"

# Add to PATH if not already there
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$installDir*") {
    Write-Host ""
    Write-Host "To add to PATH permanently, run:"
    Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"$installDir;`$env:PATH`", 'User')"
}
