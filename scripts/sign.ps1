param([Parameter(Mandatory = $true)][string]$File)
# Code-signing hook for `tauri build` (bundle.windows.signCommand). Tauri calls it for the
# app executable, the uninstaller and the installer.
#
# Signing is opt-in: without PCDIAG_SIGN_THUMBPRINT the file is left unsigned and the build
# continues. Configuration (environment variables):
#   PCDIAG_SIGN_THUMBPRINT  SHA-1 thumbprint of a code-signing certificate in Cert:\CurrentUser\My
#   PCDIAG_TIMESTAMP_URL    RFC 3161 timestamp server (default http://timestamp.digicert.com)
#   PCDIAG_SIGNTOOL         full path to signtool.exe (default: newest x64 Windows SDK copy)
$ErrorActionPreference = 'Stop'

$thumb = $env:PCDIAG_SIGN_THUMBPRINT
if ([string]::IsNullOrWhiteSpace($thumb)) {
    Write-Host "sign.ps1: PCDIAG_SIGN_THUMBPRINT not set, leaving unsigned: $File"
    exit 0
}

$signtool = $env:PCDIAG_SIGNTOOL
if ([string]::IsNullOrWhiteSpace($signtool)) {
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like '*\x64\*' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $signtool -or -not (Test-Path $signtool)) {
    Write-Error "sign.ps1: signtool.exe not found (install the Windows SDK or set PCDIAG_SIGNTOOL)"
    exit 1
}

$timestamp = $env:PCDIAG_TIMESTAMP_URL
if ([string]::IsNullOrWhiteSpace($timestamp)) { $timestamp = 'http://timestamp.digicert.com' }

& $signtool sign /sha1 $thumb /s My /fd sha256 /tr $timestamp /td sha256 /d "PC Diagnostics" $File
if ($LASTEXITCODE -ne 0) {
    Write-Error "sign.ps1: signtool failed with exit code $LASTEXITCODE for $File"
    exit $LASTEXITCODE
}
Write-Host "sign.ps1: signed $File"
