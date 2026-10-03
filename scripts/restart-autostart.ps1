#requires -Version 5.1
<#
.SYNOPSIS
    Restart the installed trofeo-lcd-winmon autostart instance.

.DESCRIPTION
    Stops any running trofeo-lcd-winmon (or legacy trofeo-screen) process and starts the
    installed copy from %LOCALAPPDATA%\TrofeoWinMon, using the same arguments as the
    Startup shortcut (unless -ExtraArgs is given).

    Use this to restart without logging off. If you rebuilt the project, refresh the
    installed binary first, e.g.:
        .\scripts\install-autostart.ps1 -Restart

.PARAMETER ExtraArgs
    Override the arguments. If omitted, the Startup shortcut's arguments are reused.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\restart-autostart.ps1
#>
param(
    [string]$ExtraArgs
)

$ErrorActionPreference = 'Stop'

$destExe = Join-Path (Join-Path $env:LOCALAPPDATA 'TrofeoWinMon') 'trofeo-lcd-winmon.exe'
if (-not (Test-Path $destExe)) {
    throw "Installed binary not found at '$destExe'. Run .\scripts\install-autostart.ps1 first."
}

# Reuse the shortcut's arguments unless explicitly overridden.
if ([string]::IsNullOrWhiteSpace($ExtraArgs)) {
    $shortcutPath = Join-Path ([Environment]::GetFolderPath('Startup')) 'Trofeo LCD WinMon.lnk'
    if (Test-Path $shortcutPath) {
        $shell = New-Object -ComObject WScript.Shell
        $ExtraArgs = $shell.CreateShortcut($shortcutPath).Arguments
    }
    if ([string]::IsNullOrWhiteSpace($ExtraArgs)) {
        $ExtraArgs = '--hide-console'
    }
}

foreach ($name in @('trofeo-lcd-winmon', 'trofeo-screen')) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force
}
Start-Sleep -Milliseconds 400

Start-Process -FilePath $destExe -ArgumentList $ExtraArgs -WindowStyle Hidden
Write-Host "Restarted: $destExe"
Write-Host "  Args: $ExtraArgs"
