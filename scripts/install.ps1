# Claude Code Notify - Windows Installation Script
# Native toast notifications for Claude Code with window activation
#
# Usage:
#   irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex

#Requires -Version 5.1

$ErrorActionPreference = "Stop"

# Global error handler for unexpected errors
trap {
    Write-Host "`n[X] Unexpected error: $_" -ForegroundColor Red
    Write-Host "[i] Please report this issue at: https://github.com/chuilishi/claude-code-notify/issues" -ForegroundColor Cyan
    exit 1
}

# Version
$VERSION = "1.0.0"

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

    # Check environment variables
    if ([string]::IsNullOrEmpty($env:USERPROFILE)) {
        Write-Err "USERPROFILE environment variable is not set"
        return $false
    }

    if ([string]::IsNullOrEmpty($env:TEMP)) {
        Write-Err "TEMP environment variable is not set"
        return $false
    }

    # Check if TEMP is writable
    $testFile = "$env:TEMP\claude-notify-test-$([guid]::NewGuid()).tmp"
    try {
        [System.IO.File]::WriteAllText($testFile, "test")
        Remove-Item $testFile -Force -ErrorAction SilentlyContinue
    }
    catch {
        Write-Err "TEMP directory is not writable: $env:TEMP"
        return $false
    }

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

    # Check if running as different user (admin elevation can change USERPROFILE)
    $currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    if ($env:USERPROFILE -notlike "*$($currentUser.Split('\')[-1])*") {
        Write-Warn "Running as elevated user. Installation path may differ from expected."
    }

    return $true
}

function Test-ExeInUse {
    param([string]$ExePath)

    if (-not (Test-Path $ExePath)) {
        return $false
    }

    # Check if process is running
    $processName = [System.IO.Path]::GetFileNameWithoutExtension($ExePath)
    $running = Get-Process -Name $processName -ErrorAction SilentlyContinue
    if ($running) {
        return $true
    }

    # Try to open file exclusively to check if locked
    try {
        $stream = [System.IO.File]::Open($ExePath, 'Open', 'ReadWrite', 'None')
        $stream.Close()
        return $false
    }
    catch {
        return $true
    }
}

function Install-Files {
    Write-Header "Downloading and installing..."

    # Paths
    $ClaudeHome = "$env:USERPROFILE\.claude"
    $NotificationsDir = "$ClaudeHome\notifications"
    $BackupDir = "$env:USERPROFILE\.config\claude-code-notify\backups"
    $TempDir = "$env:TEMP\claude-code-notify-install"
    $ExePath = "$NotificationsDir\ToastWindow.exe"

    # GitHub repository info
    $RepoOwner = "chuilishi"
    $RepoName = "claude-code-notify"
    $ReleaseUrl = "https://github.com/$RepoOwner/$RepoName/releases/latest/download/claude-code-notify.zip"

    # Check if already installed
    if (Test-Path $ExePath) {
        Write-Info "Detected existing installation, will overwrite"

        # Check if exe is in use
        if (Test-ExeInUse $ExePath) {
            Write-Err "ToastWindow.exe is currently in use"
            Write-Err "Please close any running notifications and try again"
            return $false
        }
    }

    # Create directories
    $directories = @($NotificationsDir, $BackupDir)
    foreach ($dir in $directories) {
        if (-not (Test-Path $dir)) {
            try {
                New-Item -ItemType Directory -Path $dir -Force | Out-Null
                Write-Info "Created directory: $dir"
            }
            catch {
                Write-Err "Failed to create directory: $dir"
                Write-Err "Error: $_"
                return $false
            }
        }
    }

    # Create temp directory
    try {
        if (Test-Path $TempDir) {
            Remove-Item $TempDir -Recurse -Force
        }
        New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    }
    catch {
        Write-Err "Failed to create temp directory: $TempDir"
        Write-Err "Error: $_"
        return $false
    }

    # Enable TLS 1.2 for older PowerShell versions
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    }
    catch {
        Write-Warn "Could not enable TLS 1.2, download may fail on older systems"
    }

    # Download zip
    $zipPath = "$TempDir\claude-code-notify.zip"
    Write-Info "Downloading from GitHub Releases..."

    try {
        $ProgressPreference = 'SilentlyContinue'  # Speeds up download
        Invoke-WebRequest -Uri $ReleaseUrl -OutFile $zipPath -UseBasicParsing -TimeoutSec 60
        $ProgressPreference = 'Continue'

        # Verify download
        if (-not (Test-Path $zipPath)) {
            Write-Err "Download completed but file not found"
            return $false
        }

        $fileSize = (Get-Item $zipPath).Length
        if ($fileSize -lt 1000) {
            Write-Err "Downloaded file is too small ($fileSize bytes), may be corrupted"
            return $false
        }

        Write-Success "Downloaded successfully ($([math]::Round($fileSize / 1KB, 1)) KB)"
    }
    catch [System.Net.WebException] {
        Write-Err "Network error: $($_.Exception.Message)"
        Write-Info "Please check your internet connection"
        Write-Info "If GitHub is blocked, download manually from:"
        Write-Info "  https://github.com/$RepoOwner/$RepoName/releases"
        return $false
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
        Write-Err "Failed to extract zip file"
        Write-Err "The downloaded file may be corrupted. Please try again."
        return $false
    }

    # Find extracted folder (might be nested)
    $extractedDir = $TempDir
    $foundExe = Get-ChildItem -Path $TempDir -Filter "ToastWindow.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($foundExe) {
        $extractedDir = $foundExe.DirectoryName
    }

    # Copy ToastWindow.exe
    if (Test-Path "$extractedDir\ToastWindow.exe") {
        try {
            Copy-Item "$extractedDir\ToastWindow.exe" $ExePath -Force
            Write-Success "Installed ToastWindow.exe"
        }
        catch {
            Write-Err "Failed to copy ToastWindow.exe"
            Write-Err "Error: $_"
            return $false
        }
    } else {
        Write-Err "ToastWindow.exe not found in downloaded package"
        Write-Err "The release package may be incomplete"
        return $false
    }

    # Copy assets folder if exists
    if (Test-Path "$extractedDir\assets") {
        try {
            Copy-Item "$extractedDir\assets" "$NotificationsDir\assets" -Recurse -Force
            Write-Success "Installed assets"
        }
        catch {
            Write-Warn "Failed to copy assets folder (non-critical)"
        }
    }

    # Cleanup temp
    Remove-Item $TempDir -Recurse -Force -ErrorAction SilentlyContinue

    return $true
}

function Enable-Hooks {
    Write-Header "Configuring hooks..."

    $ClaudeHome = "$env:USERPROFILE\.claude"
    $NotificationsDir = "$ClaudeHome\notifications"
    $BackupDir = "$env:USERPROFILE\.config\claude-code-notify\backups"
    $SettingsFile = "$ClaudeHome\settings.json"
    $exePath = "$NotificationsDir\ToastWindow.exe"

    # Our hooks configuration
    $ourHooks = @{
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

    # Load existing settings
    $settings = @{}
    $existingHooks = @{}

    if (Test-Path $SettingsFile) {
        # Check if file is locked
        try {
            $stream = [System.IO.File]::Open($SettingsFile, 'Open', 'ReadWrite', 'None')
            $stream.Close()
        }
        catch {
            Write-Err "settings.json is locked by another process"
            Write-Err "Please close any applications that may be using it"
            return $false
        }

        try {
            $existingContent = Get-Content $SettingsFile -Raw -ErrorAction Stop
            if ($existingContent) {
                $existingObj = $existingContent | ConvertFrom-Json -ErrorAction Stop

                # Preserve all non-hook settings
                $existingObj.PSObject.Properties | ForEach-Object {
                    if ($_.Name -eq "hooks") {
                        # Save existing hooks for merging
                        if ($_.Value) {
                            $_.Value.PSObject.Properties | ForEach-Object {
                                $existingHooks[$_.Name] = $_.Value
                            }
                        }
                    } else {
                        $settings[$_.Name] = $_.Value
                    }
                }
            }

            # Backup existing settings
            try {
                $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
                $backupPath = "$BackupDir\settings.$timestamp.json"
                Copy-Item $SettingsFile $backupPath -Force
                Write-Info "Backed up settings to: $backupPath"
            }
            catch {
                Write-Warn "Could not create backup (non-critical)"
            }
        }
        catch {
            Write-Warn "Could not parse existing settings.json, will create new one"

            # Backup corrupted file
            try {
                $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
                $backupPath = "$BackupDir\settings.corrupted.$timestamp.json"
                Copy-Item $SettingsFile $backupPath -Force
                Write-Info "Original file backed up to: $backupPath"
                Write-Info "To restore: copy the backup file back to settings.json"
            }
            catch {
                Write-Warn "Could not backup the corrupted file"
            }
        }
    }

    # Merge hooks: our hooks + existing hooks (our hooks take priority for same event types)
    $mergedHooks = @{}

    # First add existing hooks
    foreach ($key in $existingHooks.Keys) {
        $mergedHooks[$key] = $existingHooks[$key]
    }

    # Then add/override with our hooks
    foreach ($key in $ourHooks.Keys) {
        if ($mergedHooks.ContainsKey($key)) {
            # Event type exists, need to merge the hook arrays
            $existingArray = @($mergedHooks[$key])
            $ourArray = @($ourHooks[$key])

            # Remove any existing claude-code-notify hooks (by checking command contains ToastWindow)
            $filteredExisting = @()
            foreach ($item in $existingArray) {
                $isOurs = $false
                if ($item.hooks) {
                    foreach ($h in $item.hooks) {
                        if ($h.command -and $h.command -like "*ToastWindow*") {
                            $isOurs = $true
                            break
                        }
                    }
                }
                if (-not $isOurs) {
                    $filteredExisting += $item
                }
            }

            # Combine: our hooks first, then user's other hooks
            if ($filteredExisting.Count -gt 0) {
                $combined = [System.Collections.ArrayList]::new()
                foreach ($item in $ourArray) { [void]$combined.Add($item) }
                foreach ($item in $filteredExisting) { [void]$combined.Add($item) }
                $mergedHooks[$key] = $combined.ToArray()
            } else {
                $mergedHooks[$key] = $ourArray
            }
        } else {
            $mergedHooks[$key] = $ourHooks[$key]
        }
    }

    $settings["hooks"] = $mergedHooks

    # Ensure directory exists
    if (-not (Test-Path $ClaudeHome)) {
        try {
            New-Item -ItemType Directory -Path $ClaudeHome -Force | Out-Null
        }
        catch {
            Write-Err "Failed to create directory: $ClaudeHome"
            return $false
        }
    }

    # Write settings
    try {
        $settings | ConvertTo-Json -Depth 10 | Set-Content $SettingsFile -Encoding UTF8
        Write-Success "Hooks configured in: $SettingsFile"
    }
    catch {
        Write-Err "Failed to write settings.json"
        Write-Err "Error: $_"
        return $false
    }

    return $true
}

function Show-Complete {
    $NotificationsDir = "$env:USERPROFILE\.claude\notifications"
    $SettingsFile = "$env:USERPROFILE\.claude\settings.json"

    Write-Host @"

========================================
  Installation Complete!
========================================

"@ -ForegroundColor Green

    Write-Host "How it works:" -ForegroundColor White
    Write-Host "  - When Claude finishes a task, a notification appears" -ForegroundColor Gray
    Write-Host "  - When Claude needs input, a different notification appears" -ForegroundColor Gray
    Write-Host "  - Left-click to jump back to your terminal/editor" -ForegroundColor Gray
    Write-Host "  - Right-click or X to dismiss" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Installed to:" -ForegroundColor White
    Write-Host "  $NotificationsDir" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Configuration:" -ForegroundColor White
    Write-Host "  $SettingsFile" -ForegroundColor Gray
    Write-Host ""
}

# Main
function Main {
    Show-Banner

    if (-not (Test-Prerequisites)) {
        exit 1
    }

    if (-not (Install-Files)) {
        exit 1
    }

    if (-not (Enable-Hooks)) {
        exit 1
    }

    Show-Complete
    exit 0
}

Main
