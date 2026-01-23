# Claude Code Notify - Windows Installation Script
# Native toast notifications for Claude Code with window activation
#
# Usage:
#   irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex

#Requires -Version 5.1

param(
    [switch]$Uninstall,
    [switch]$Silent,
    [string]$Branch = "main"
)

$ErrorActionPreference = "Stop"

# Version
$VERSION = "1.0.0"

# Paths (following code-notify convention)
$ClaudeHome = "$env:USERPROFILE\.claude"
$NotificationsDir = "$ClaudeHome\notifications"
$BackupDir = "$env:USERPROFILE\.config\claude-code-notify\backups"
$SettingsFile = "$ClaudeHome\settings.json"
$LogsDir = "$ClaudeHome\logs"
$TempDir = "$env:TEMP\claude-code-notify-install"

# GitHub repository info
$RepoOwner = "chuilishi"
$RepoName = "claude-code-notify"
$ReleaseUrl = "https://github.com/$RepoOwner/$RepoName/releases/latest/download/claude-code-notify.zip"

# Colors
function Write-Success { param([string]$Message) Write-Host "[OK] $Message" -ForegroundColor Green }
function Write-Info { param([string]$Message) Write-Host "[i] $Message" -ForegroundColor Cyan }
function Write-Warn { param([string]$Message) Write-Host "[!] $Message" -ForegroundColor Yellow }
function Write-Err { param([string]$Message) Write-Host "[X] $Message" -ForegroundColor Red }
function Write-Header { param([string]$Message) Write-Host "`n$Message" -ForegroundColor White }

function Show-Banner {
    Write-Host @"

 ========================================
   Claude Code Notify v$VERSION
   Native Windows Toast Notifications
 ========================================

"@ -ForegroundColor Cyan
}

function Test-Prerequisites {
    Write-Header "Checking prerequisites..."

    # Check Windows version
    $osVersion = [System.Environment]::OSVersion.Version
    if ($osVersion.Major -lt 10) {
        Write-Err "Windows 10 or higher is required"
        return $false
    }
    Write-Success "Windows $($osVersion.Major).$($osVersion.Minor)"

    # Check PowerShell version
    $psVersion = $PSVersionTable.PSVersion
    if ($psVersion.Major -lt 5) {
        Write-Err "PowerShell 5.1 or higher is required"
        return $false
    }
    Write-Success "PowerShell $psVersion"

    return $true
}

function Install-Files {
    Write-Header "Downloading and installing..."

    # Create directories
    $directories = @($NotificationsDir, $BackupDir, $LogsDir)
    foreach ($dir in $directories) {
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
            Write-Info "Created directory: $dir"
        }
    }

    # Create temp directory
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

    # Download zip
    $zipPath = "$TempDir\claude-code-notify.zip"
    Write-Info "Downloading from GitHub Releases..."

    try {
        Invoke-WebRequest -Uri $ReleaseUrl -OutFile $zipPath -UseBasicParsing
        Write-Success "Downloaded successfully"
    }
    catch {
        Write-Err "Failed to download: $_"
        Write-Info "Please download manually from: https://github.com/$RepoOwner/$RepoName/releases"
        return $false
    }

    # Extract zip
    Write-Info "Extracting files..."
    try {
        Expand-Archive -Path $zipPath -DestinationPath $TempDir -Force
        Write-Success "Extracted successfully"
    }
    catch {
        Write-Err "Failed to extract: $_"
        return $false
    }

    # Find extracted folder (might be nested)
    $extractedDir = $TempDir
    $exePath = Get-ChildItem -Path $TempDir -Filter "ToastWindow.exe" -Recurse | Select-Object -First 1
    if ($exePath) {
        $extractedDir = $exePath.DirectoryName
    }

    # Copy ToastWindow.exe
    if (Test-Path "$extractedDir\ToastWindow.exe") {
        Copy-Item "$extractedDir\ToastWindow.exe" "$NotificationsDir\ToastWindow.exe" -Force
        Write-Success "Installed ToastWindow.exe"
    } else {
        Write-Err "ToastWindow.exe not found in zip"
        return $false
    }

    # Copy assets folder if exists
    if (Test-Path "$extractedDir\assets") {
        Copy-Item "$extractedDir\assets" "$NotificationsDir\assets" -Recurse -Force
        Write-Success "Installed assets"
    }

    # Cleanup temp
    Remove-Item $TempDir -Recurse -Force -ErrorAction SilentlyContinue

    return $true
}

function Enable-Hooks {
    Write-Header "Configuring hooks..."

    $exePath = "$NotificationsDir\ToastWindow.exe" -replace '\\', '\\\\'

    # New hooks configuration (following code-notify format)
    $hooks = @{
        UserPromptSubmit = @(
            @{
                matcher = ""
                hooks = @(
                    @{
                        type = "command"
                        command = "$exePath --save"
                        timeout = 5
                    }
                )
            }
        )
        Notification = @(
            @{
                matcher = ""
                hooks = @(
                    @{
                        type = "command"
                        command = "$exePath --input"
                        timeout = 10
                    }
                )
            }
        )
        Stop = @(
            @{
                matcher = ""
                hooks = @(
                    @{
                        type = "command"
                        command = "$exePath --notify"
                        timeout = 10
                    }
                )
            }
        )
    }

    # Load existing settings and preserve non-hook settings
    $settings = @{}
    if (Test-Path $SettingsFile) {
        try {
            $existingContent = Get-Content $SettingsFile -Raw -ErrorAction SilentlyContinue
            if ($existingContent) {
                $existingObj = $existingContent | ConvertFrom-Json -ErrorAction SilentlyContinue
                if ($existingObj) {
                    $existingObj.PSObject.Properties | ForEach-Object {
                        if ($_.Name -ne "hooks") {
                            $settings[$_.Name] = $_.Value
                        }
                    }
                }
            }
            # Backup existing settings with timestamp
            $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
            $backupPath = "$BackupDir\settings.$timestamp.json"
            Copy-Item $SettingsFile $backupPath -Force -ErrorAction SilentlyContinue
            Write-Info "Backed up settings to: $backupPath"
        }
        catch {
            Write-Warn "Could not parse existing settings.json"
        }
    }

    # Add hooks
    $settings["hooks"] = $hooks

    # Ensure directory exists
    if (-not (Test-Path $ClaudeHome)) {
        New-Item -ItemType Directory -Path $ClaudeHome -Force | Out-Null
    }

    # Write settings
    $settings | ConvertTo-Json -Depth 10 | Set-Content $SettingsFile -Encoding UTF8
    Write-Success "Hooks configured in: $SettingsFile"

    return $true
}

function Disable-Hooks {
    Write-Header "Disabling hooks..."

    if (-not (Test-Path $SettingsFile)) {
        Write-Warn "Settings file not found"
        return $true
    }

    try {
        $settings = Get-Content $SettingsFile -Raw | ConvertFrom-Json
        if ($settings.hooks) {
            # Remove hooks property
            $newSettings = @{}
            $settings.PSObject.Properties | ForEach-Object {
                if ($_.Name -ne "hooks") {
                    $newSettings[$_.Name] = $_.Value
                }
            }
            $newSettings | ConvertTo-Json -Depth 10 | Set-Content $SettingsFile -Encoding UTF8
            Write-Success "Hooks disabled"
        } else {
            Write-Info "Hooks were not enabled"
        }
    }
    catch {
        Write-Err "Failed to disable hooks: $_"
        return $false
    }

    return $true
}

function Uninstall-ClaudeNotify {
    Write-Header "Uninstalling Claude Code Notify..."

    # Disable hooks first
    Disable-Hooks

    # Remove notification files
    if (Test-Path $NotificationsDir) {
        Remove-Item $NotificationsDir -Recurse -Force
        Write-Success "Removed: $NotificationsDir"
    }

    Write-Success "Claude Code Notify uninstalled!"
    Write-Info "Note: Your settings in $ClaudeHome were preserved (hooks removed)"
}

function Show-Complete {
    Write-Host @"

========================================
  Installation Complete!
========================================

"@ -ForegroundColor Green

    Write-Host "How it works:" -ForegroundColor White
    Write-Host "  - When Claude finishes a task, a notification appears (orange border)" -ForegroundColor Gray
    Write-Host "  - When Claude needs input, a different notification appears (yellow border)" -ForegroundColor Gray
    Write-Host "  - Left-click to jump back to your terminal/editor" -ForegroundColor Gray
    Write-Host "  - Right-click or X to dismiss" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Installed to:" -ForegroundColor White
    Write-Host "  $NotificationsDir" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Configuration:" -ForegroundColor White
    Write-Host "  $SettingsFile" -ForegroundColor Gray
    Write-Host ""
    Write-Host "To uninstall:" -ForegroundColor White
    Write-Host "  irm https://raw.githubusercontent.com/$RepoOwner/$RepoName/main/scripts/install.ps1 | iex -Uninstall" -ForegroundColor Cyan
    Write-Host ""
}

# Main
function Main {
    Show-Banner

    if ($Uninstall) {
        Uninstall-ClaudeNotify
        return
    }

    if (-not (Test-Prerequisites)) {
        exit 1
    }

    if (-not (Install-Files)) {
        exit 1
    }

    if (-not (Enable-Hooks)) {
        exit 1
    }

    if (-not $Silent) {
        Show-Complete
    }
}

Main
