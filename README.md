# Claude Code Notify

原生 Windows Toast 通知，用于 Claude Code 任务完成提醒。点击通知可跳转回对应的 Claude Code 运行窗口（编辑器/终端）。

基于 Claude Code 的 hook 功能。

![Demo](assets/demo.gif)

## 特性

- 原生 Windows Toast 通知
- 点击通知自动激活并跳转回原窗口
- 支持 Windows Terminal 标签页精确切换
- 支持 VSCode、Cursor、JetBrains IDE 等编辑器
- 自动提取调用应用图标

## 系统要求

- Windows 10/11

## 一键安装

打开 PowerShell，运行：

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-code-notify/main/scripts/install.ps1 | iex
```

安装脚本会自动：
1. 下载 ToastWindow.exe 和资源文件到 `~/.claude/scripts/`
2. 配置 `~/.claude/settings.json` 中的 hooks（会备份原有配置）


将 `你的用户名` 替换为你的 Windows 用户名。

## 使用方法

回答结束后，右下角弹出通知：
- **左键点击**：跳转回 Claude Code 所在窗口
- **右键点击** 或 **点击 ×**：关闭通知

支持几乎所有的终端和代码编辑器。

## 卸载

1. 删除 `~/.claude/scripts/ToastWindow.exe` 和 `assets` 文件夹
2. 编辑 `~/.claude/settings.json`，删除 `hooks` 部分（或恢复 `settings.json.backup`）

## 工作原理

1. `UserPromptSubmit` hook：用户发送消息时，保存当前窗口状态
2. `Stop` hook：Claude 完成任务时，显示通知
3. 点击通知：使用保存的窗口句柄激活原窗口

~~音效真的很好听~~

## 许可证

MIT
