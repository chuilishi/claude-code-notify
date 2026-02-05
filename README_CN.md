<div align="center">

# 🔔 Claude Code Notify

[![English](https://img.shields.io/badge/English-Click_to_view-blue?style=for-the-badge)](README.md)

**Claude Code 原生 Windows Toast 通知**

![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows&logoColor=white)
![PowerShell](https://img.shields.io/badge/PowerShell-5.1+-5391FE?logo=powershell&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-green)

<img src="assets/demo.gif" width="450">

*点击通知即可跳转回 Claude Code 窗口*

</div>

---

## ✨ 特性

- 🔔 **原生 Toast 通知** — 干净、系统级的通知体验
- 🎯 **一键返回** — 点击通知跳转回终端/编辑器
- 🖥️ **广泛兼容** — 支持 VSCode、Cursor、JetBrains、Windows Terminal 等
- 🔄 **标签页感知** — 支持 Windows Terminal 标签页精确切换
- 🎨 **自动图标** — 自动提取调用应用的图标

---

## 🚀 一键安装

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex
```

<details>
<summary>安装脚本做了什么？</summary>

1. 下载 `ToastWindow.exe` 和资源文件到 `~/.claude/notifications/`
2. 配置 `~/.claude/settings.json` 中的 hooks（会备份原有配置）

</details>

---

## 📖 使用方法

Claude 回答结束后，右下角弹出通知：

| 操作 | 效果 |
|------|------|
| **左键点击** | 跳转回 Claude Code 窗口 |
| **右键点击** / **点击 ×** | 关闭通知 |

---

## 🗑️ 卸载

1. 删除 `~/.claude/notifications/` 文件夹
2. 编辑 `~/.claude/settings.json`，删除 `hooks` 部分

---

<details>
<summary><b>⚙️ 工作原理</b></summary>

<br>

| Hook | 触发时机 | 动作 |
|------|---------|------|
| `UserPromptSubmit` | 发送消息时 | 保存当前窗口状态 |
| `Stop` | Claude 完成时 | 显示通知 |
| *点击通知* | — | 激活保存的窗口 |

</details>

---

<div align="center">

MIT License

</div>
