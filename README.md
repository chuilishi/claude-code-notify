<div align="center">

# 🔔 Claude Code Notify

[![中文](https://img.shields.io/badge/中文-点击查看-blue?style=for-the-badge)](README_CN.md)

**Native Windows Toast notifications for Claude Code**

![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows&logoColor=white)
![PowerShell](https://img.shields.io/badge/PowerShell-5.1+-5391FE?logo=powershell&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-green)

<img src="assets/demo.gif" width="450">

*Click the notification to jump back to your Claude Code window*

</div>

---

## ✨ Features

- 🔔 **Native Windows Toast** — Clean, system-integrated notifications
- 🎯 **One-Click Return** — Click to jump back to your terminal/editor
- 🖥️ **Wide Compatibility** — VSCode, Cursor, JetBrains, Windows Terminal, and more
- 🔄 **Tab-Aware** — Supports Windows Terminal tab switching
- 🎨 **Auto Icon** — Extracts the calling application's icon

---

## 🚀 Installation

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex
```

<details>
<summary>What does the script do?</summary>

1. Downloads `ToastWindow.exe` and assets to `~/.claude/notifications/`
2. Configures hooks in `~/.claude/settings.json` (backs up existing config)

</details>

---

## 📖 Usage

After Claude finishes responding, a notification appears:

| Action | Result |
|--------|--------|
| **Left-click** | Jump back to Claude Code window |
| **Right-click** / **X** | Dismiss notification |

---

## 🗑️ Uninstall

1. Delete `~/.claude/notifications/`
2. Remove `hooks` section from `~/.claude/settings.json`

---

<details>
<summary><b>⚙️ How It Works</b></summary>

<br>

| Hook | Trigger | Action |
|------|---------|--------|
| `UserPromptSubmit` | You send a message | Saves current window state |
| `Stop` | Claude finishes | Shows notification |
| *Click notification* | — | Activates saved window |

</details>

---

<div align="center">

MIT License

</div>
