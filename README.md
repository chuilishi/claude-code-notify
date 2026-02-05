# Claude Code Notify

[![中文](https://img.shields.io/badge/中文-点击查看-blue?style=for-the-badge)](README_CN.md)

Native Windows Toast notifications for Claude Code task completion. Click the notification to jump back to your Claude Code window (editor/terminal).

Based on Claude Code's hook feature.

![Demo](assets/demo.gif)

## Features

- Native Windows Toast notifications
- Click to activate and jump back to the original window
- Supports Windows Terminal tab switching
- Supports VSCode, Cursor, JetBrains IDEs, and more
- Automatically extracts the calling application's icon

## Requirements

- Windows 10/11
- PowerShell 5.1+

## Installation

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex
```

The script will:
1. Download ToastWindow.exe and assets to `~/.claude/notifications/`
2. Configure hooks in `~/.claude/settings.json` (backs up existing config)

## Usage

After Claude finishes responding, a notification appears in the bottom-right corner:
- **Left-click**: Jump back to the Claude Code window
- **Right-click** or **click X**: Dismiss the notification

Works with almost all terminals and code editors.

## Uninstall

1. Delete the `~/.claude/notifications/` folder
2. Edit `~/.claude/settings.json` and remove the `hooks` section

## How It Works

1. `UserPromptSubmit` hook: Saves the current window state when you send a message
2. `Stop` hook: Shows a notification when Claude finishes
3. Click notification: Activates the saved window handle

## License

MIT
