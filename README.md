<div align="center">

# 🔔 Claude Code Notify

[![中文](https://img.shields.io/badge/中文-点击查看-blue?style=for-the-badge)](README_CN.md)

**Native Windows Toast notifications for Claude Code**

![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows&logoColor=white)
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

```bash
claude plugin marketplace add chuilishi/claude-code-notify
claude plugin install claude-code-notify@claude-code-notify
```

That's it. Restart Claude Code and notifications will work automatically.

---

## 📖 Usage

After Claude finishes responding, a notification appears:

| Action | Result |
|--------|--------|
| **Left-click** | Jump back to Claude Code window |
| **Right-click** / **X** | Dismiss notification |

---

## 🗑️ Uninstall

```bash
claude plugin uninstall claude-code-notify
```

---

<details>
<summary><b>⚙️ How It Works</b></summary>

<br>

This project uses Claude Code's **plugin system** to register hooks automatically — no manual `settings.json` editing required.

| Hook | Trigger | Action |
|------|---------|--------|
| `UserPromptSubmit` | You send a message | Saves current window state |
| `Stop` | Claude finishes | Shows "Task completed" notification |
| `Notification` | Claude needs input | Shows "Input required" notification |
| *Click notification* | — | Activates saved window |

</details>

---

<div align="center">

MIT License

</div>
