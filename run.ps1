# Download and run The European Correspondent TUI (Windows, no permanent install)
# Usage: irm https://raw.githubusercontent.com/zebra-pig/tui-europeancorrespondent/main/run.ps1 | iex

$repo = "zebra-pig/tui-europeancorrespondent"
$binary = "tui-europeancorrespondent"

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") { "aarch64" } else { "x86_64" }
$asset = "$binary-$arch-pc-windows-msvc.exe"
$url = "https://github.com/$repo/releases/latest/download/$asset"

$installDir = Join-Path $env:TEMP "european-correspondent"
$binPath = Join-Path $installDir "$binary.exe"

New-Item -ItemType Directory -Force -Path $installDir | Out-Null

Write-Host "The European Correspondent - Terminal Edition"
Write-Host "=============================================="
Write-Host ""
Write-Host "Platform: Windows $arch"
Write-Host ""

Invoke-WebRequest -Uri $url -OutFile $binPath -UseBasicParsing

Write-Host ""

& $binPath @args
