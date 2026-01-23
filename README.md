# Claude Code Notify

原生 Windows toast 通知，用于 Claude Code 任务完成提醒。点击通知可跳转回对应的Claude Code运行程序(编辑器/终端)
基于 Claude Code的hook功能

![Demo](assets/demo.gif)

## 系统要求

- Windows 10/11

### 安装

1. 复制ToastWindow.exe和assets文件夹到 .claude/scripts 目录

2. 在 `.claude/settings.json` 中添加 hooks：

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "C:\\Users\\你的用户名\\.claude\\scripts\\ToastWindow.exe --save",
            "timeout": 5
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "C:\\Users\\你的用户名\\.claude\\scripts\\ToastWindow.exe --notify",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

将 `你的用户名` 替换为你的 Windows 用户名。

## 使用方法

回答结束之后,右下角弹出通知,左键可以打开对应的Claude Code窗口,右键或点击右上角×可以关闭窗口

支持几乎所有的终端和代码编辑器

~~音效真的很好听~~

## 许可证

MIT

