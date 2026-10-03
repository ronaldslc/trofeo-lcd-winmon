#requires -Version 5.1
<#
.SYNOPSIS
    Remove the trofeo-lcd-winmon autostart entry.

.DESCRIPTION
    Deletes the Startup shortcut and, optionally, the copied binary directory.
    No administrator rights are required.

.PARAMETER KeepBinary
    Do not delete %LOCALAPPDATA%\TrofeoWinMon (only remove the shortcut).

.PARAMETER StopRunning
    Also stop any running trofeo-lcd-winmon process.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\uninstall-autostart.ps1
#>
param(
    [switch]$KeepBinary,
    [switch]$StopRunning
)

$ErrorActionPreference = 'Stop'

$startup = [Environment]::GetFolderPath('Startup')
$shortcutPath = Join-Path $startup 'Trofeo LCD WinMon.lnk'

if (Test-Path $shortcutPath) {
    Remove-Item -Force $shortcutPath
    Write-Host "Removed shortcut: $shortcutPath"
} else {
    Write-Host "No Startup shortcut found (nothing to remove)."
}

if ($StopRunning) {
    Get-Process -Name 'trofeo-lcd-winmon' -ErrorAction SilentlyContinue | Stop-Process -Force
    Write-Host "Stopped running trofeo-lcd-winmon process(es)."
}

if (-not $KeepBinary) {
    $destDir = Join-Path $env:LOCALAPPDATA 'TrofeoWinMon'
    if (Test-Path $destDir) {
        Remove-Item -Recurse -Force $destDir
        Write-Host "Removed $destDir"
    }
}

Write-Host "Done."
