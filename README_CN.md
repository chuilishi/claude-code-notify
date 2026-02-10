<div align="center">

# 🔔 Claude Code Notify

**Claude Code 原生 Windows Toast 通知**

**[📖 English Documentation](README.md)**

![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows&logoColor=white)
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

## 🚀 安装

```bash
claude plugin marketplace add chuilishi/claude-code-notify
claude plugin install claude-code-notify@claude-code-notify
```

就这样。重启 Claude Code 即可自动生效。

---

## 📖 使用方法

Claude 回答结束后，右下角弹出通知：

| 操作 | 效果 |
|------|------|
| **左键点击** | 跳转回 Claude Code 窗口 |
| **右键点击** / **点击 ×** | 关闭通知 |

---

## 🗑️ 卸载

```bash
claude plugin uninstall claude-code-notify
```

---

<details>
<summary><b>⚙️ 工作原理</b></summary>

<br>

### 插件系统

本项目使用 Claude Code 的**插件系统**自动注册 hooks，无需手动编辑 `settings.json`。Hooks 定义在 `hooks/hooks.json` 中，Claude Code 启动时自动发现并加载。

### Hook 流程

| Hook | 触发时机 | 动作 |
|------|---------|------|
| `UserPromptSubmit` | 发送消息时 | 保存当前窗口句柄、活动标签页、调用应用图标 |
| `Stop` | Claude 完成时 | 显示"任务完成"通知（橙色边框） |
| `Notification` | Claude 需要输入时 | 显示"需要输入"通知（黄色边框） |
| *点击通知* | — | 激活保存的窗口并切换到正确的标签页 |

### 会话隔离

每个 Claude Code 会话有唯一的 `session_id`（通过 stdin JSON 接收）。状态按会话存储在 `%TEMP%\claude-notify-{session_id}.txt`，多个 Claude 实例互不干扰。

### Windows Terminal 标签页切换

在 Windows Terminal 中运行时，仅将窗口提到前台是不够的——用户可能已经切换到其他标签页。本项目使用 **Windows UI Automation API** 实现精确切换：

1. 检测前台窗口是否为 Windows Terminal（通过 `CASCADIA_HOSTING_WINDOW_CLASS` 窗口类名识别）
2. 枚举所有标签页项，记录发送消息时**当前选中标签页的 RuntimeId**
3. 点击通知时，找到匹配 RuntimeId 的标签页，调用 `IUIAutomationSelectionItemPattern::Select()` 切换回去

### 调用应用图标提取

通知显示的是你正在使用的应用的图标（VSCode、Cursor、JetBrains IDE 等），而不是通用图标。实现方式是在发送消息时**向上遍历进程树**：

- 跳过已知的 shell/运行时进程（cmd、powershell、bash、node、python、uv 等）
- 识别已知应用：**VSCode**、**Cursor**、**Windsurf**、**Codium**、**JetBrains IDE**（IntelliJ、WebStorm、PyCharm、Rider、GoLand、CLion）、**Windows Terminal**、**ConEmu**、**Tabby**、**WezTerm**
- 通过 `ExtractIconExW()` 提取应用图标并显示在通知中

### 窗口激活

Windows 限制了 `SetForegroundWindow()`——后台进程不能直接抢占焦点。本项目使用多种技术绕过限制：

- `AllowSetForegroundWindow(ASFW_ANY)` 允许前台切换
- ALT 键模拟技巧，满足 Windows 的焦点保护机制
- 线程输入关联（`AttachThreadInput`）连接当前线程、前台线程和目标线程
- 组合使用 `SetWindowPos` + `BringWindowToTop` + `SwitchToThisWindow` + `SetForegroundWindow` 确保可靠激活

### 通知堆叠

多个通知垂直堆叠（Telegram 风格），互不遮挡：

- 所有通知共享类名 `ClaudeCodeToast`，通过 `EnumWindows` 相互发现
- 新通知出现在已有通知上方；某个关闭时，其他通知平滑下移
- 只有最底部的通知启动自动消失计时器；上方的通知等待
- 鼠标悬停在**任意**通知上，**所有**通知的计时器都会暂停

### 非侵入式显示

通知创建时使用 `WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_LAYERED` 窗口样式：

- 永远不会抢占当前窗口的焦点
- 始终显示在所有窗口之上
- 支持平滑的淡出动画（通过 Alpha 混合实现）

</details>

---

<div align="center">

MIT License

</div>
