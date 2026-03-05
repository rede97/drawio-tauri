# Silent MSI installation for Microsoft Intune deployment
# Intune install command: powershell.exe -ExecutionPolicy Bypass -File install.ps1
#
# Finds the MSI in the same directory as this script and installs it silently.
# Returns the msiexec exit code so Intune can determine success/failure.

$ErrorActionPreference = 'Stop'

# MSI installation requires administrator privileges
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error 'This script must be run as Administrator.'
    exit 1
}

$msi = Get-ChildItem -Path $PSScriptRoot -Filter '*.msi' | Select-Object -First 1

if (-not $msi) {
    Write-Error 'No MSI file found in the package directory.'
    exit 1
}

Write-Host "Installing $($msi.Name) ..."

$process = Start-Process -FilePath 'msiexec.exe' `
    -ArgumentList "/i `"$($msi.FullName)`" /qn /norestart" `
    -Wait -PassThru

if ($process.ExitCode -eq 0) {
    Write-Host 'Installation completed successfully.'
} else {
    Write-Error "msiexec exited with code $($process.ExitCode)."
}

exit $process.ExitCode
