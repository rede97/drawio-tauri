# Intune detection script
#
# Intune runs this script to decide whether the app is already installed.
#   - Exit 0 + stdout output  => app detected (installed)
#   - Exit 1 (or no output)   => app not detected
#
# Checks both native and WOW6432Node registry uninstall keys so the
# detection works for both 64-bit and 32-bit MSI installations.

$uninstallPaths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
)

$app = Get-ItemProperty $uninstallPaths -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'draw.io*' } |
    Select-Object -First 1

if ($app) {
    Write-Host "Detected: $($app.DisplayName) $($app.DisplayVersion)"
    exit 0
}

exit 1
