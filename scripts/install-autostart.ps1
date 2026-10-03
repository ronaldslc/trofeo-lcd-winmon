#requires -Version 5.1
<#
.SYNOPSIS
    Install trofeo-lcd-winmon into the current user's Startup folder.

.DESCRIPTION
    Copies the release binary to a stable location (%LOCALAPPDATA%\TrofeoWinMon) and
    creates a shortcut in shell:startup so it launches at every logon, hidden.

    No administrator rights are required. The shortcut keeps running in the user's
    interactive session, which is required because the app captures the desktop.

.PARAMETER ExtraArgs
    Arguments passed to trofeo-lcd-winmon (default: "--hide-console").

.PARAMETER Restart
    Stop any running instance first (so the binary can be replaced), then start the
    freshly installed copy. Use this after a rebuild to pick up the new build.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1 -Restart
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1 -Restart -ExtraArgs "--hide-console --quality 85"
#>
param(
    [string]$ExtraArgs = "--hide-console",
    [switch]$Restart
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$sourceExe = Join-Path $repoRoot 'target\release\trofeo-lcd-winmon.exe'

if (-not (Test-Path $sourceExe)) {
    throw "Release binary not found at '$sourceExe'. Build it first:  cargo build --release"
}

# Current + legacy process names.
$processNames = @('trofeo-lcd-winmon', 'trofeo-screen')

if ($Restart) {
    foreach ($name in $processNames) {
        Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force
    }
    Start-Sleep -Milliseconds 400
}

$destDir = Join-Path $env:LOCALAPPDATA 'TrofeoWinMon'
$destExe = Join-Path $destDir 'trofeo-lcd-winmon.exe'
New-Item -ItemType Directory -Force -Path $destDir | Out-Null
try {
    Copy-Item -Force $sourceExe $destExe
} catch {
    throw "Could not replace '$destExe' (it may be running). Re-run with -Restart, or stop trofeo-lcd-winmon first."
}

$startup = [Environment]::GetFolderPath('Startup')
$shortcutPath = Join-Path $startup 'Trofeo LCD WinMon.lnk'

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $destExe
$shortcut.Arguments = $ExtraArgs
$shortcut.WorkingDirectory = $destDir
$shortcut.WindowStyle = 7  # minimized; --hide-console hides it entirely
$shortcut.Description = 'Trofeo Vision 11.3 second-monitor bridge'
$shortcut.Save()

Write-Host "Installed."
Write-Host "  Binary   : $destExe"
Write-Host "  Shortcut : $shortcutPath"
Write-Host "  Args     : $ExtraArgs"

if ($Restart) {
    Start-Process -FilePath $destExe -ArgumentList $ExtraArgs -WindowStyle Hidden
    Write-Host "Restarted (stopped old instance, started new one)."
} else {
    Write-Host ""
    Write-Host "It will start at your next logon. To (re)start it now, use:"
    Write-Host "  powershell -ExecutionPolicy Bypass -File .\scripts\restart-autostart.ps1"
}
