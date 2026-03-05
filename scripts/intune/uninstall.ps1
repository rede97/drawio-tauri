# Silent MSI uninstallation for Microsoft Intune deployment
# Intune uninstall command: powershell.exe -ExecutionPolicy Bypass -File uninstall.ps1
#
# Locates the draw.io entry in the registry uninstall keys and removes it
# via msiexec.
#
# NOTE: This intentionally avoids the deprecated wmic.exe and
# Get-WmiObject cmdlet. Registry lookups are the recommended approach
# for Intune—they are fast and do not trigger MSI self-repair the way
# Win32_Product queries can.

$ErrorActionPreference = 'Stop'

# MSI uninstallation requires administrator privileges
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error 'This script must be run as Administrator.'
    exit 1
}

$uninstallPaths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
)

$app = Get-ItemProperty $uninstallPaths -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'draw.io*' } |
    Select-Object -First 1

if ($app -and $app.UninstallString) {
    # Extract the MSI product code ({GUID}) from the uninstall string
    if ($app.UninstallString -match '\{[0-9A-Fa-f\-]+\}') {
        $productCode = $Matches[0]
        Write-Host "Uninstalling draw.io ($productCode) ..."
        $process = Start-Process -FilePath 'msiexec.exe' `
            -ArgumentList "/x $productCode /qn /norestart" `
            -Wait -PassThru
        exit $process.ExitCode
    }
}

Write-Host 'draw.io installation not found; nothing to uninstall.'
exit 0
