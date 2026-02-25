# Spellpaste Design v1

## 概念

全局文本变换工具。用户在任意应用中选中文本，按快捷键触发，弹出命令面板选择 spell，原位替换成变换后的结果。

## 核心流程

```
用户选中文本（可选）→ 按快捷键 → 弹出命令面板 → 搜索/选择 spell → 结果替换原位文本
```

## Spell 类型

| 类型 | 例子 | stdin | 说明 |
|------|------|-------|------|
| 静态生成型 | `email`、`date` | 无 | 输出固定内容 |
| 动态生成型 | `uuid`、`random` | 无/忽略 | 运行脚本，输出结果 |
| 变换型 | `translate`、`uppercase` | 选中的文本 | 读取输入，输出变换结果 |

行为统一：**永远替换**（无选中文本时 = 在光标处插入）。

## Spell 配置（初始：文件夹方案）

```
~/.spellpaste/spells/
  email.txt          ← 静态：文件内容直接作为输出
  uuid.py            ← 脚本：stdout 是输出结果
  translate.py       ← 变换脚本：stdin 读取选中文本，stdout 输出结果
```

### 脚本接口约定
- 选中文本 → 脚本 `stdin`
- 脚本 `stdout` → 替换内容
- 无选中文本时 → `stdin` 为空，脚本自行处理

## UI 交互

- 命令面板风格（类似 Raycast / VS Code Command Palette）
- 顶部搜索框，支持模糊搜索 spell 名称
- 列表显示匹配结果，回车应用

## 测试策略

### 自动化测试
OS 交互层（剪贴板、快捷键、窗口）全部抽象为 trait，业务逻辑依赖接口而非具体实现，测试时注入 mock。

```
自动化测试覆盖：
  - Rust 单元测试：spell 扫描、spell 执行、剪贴板流程逻辑、窗口状态逻辑
  - 前端 Vitest：搜索过滤、spell 列表渲染
```

### 手动测试
OS 真实行为需要手动验证，文档维护在 `tests/manual/` 下：

```
tests/
  manual/
    clipboard.md     ← 剪贴板保存/恢复在各 app 中的行为
    hotkey.md        ← 全局快捷键在各场景下的响应
    window.md        ← 窗口弹出位置、焦点、层级行为
    cross-app.md     ← 在不同 app（浏览器、Electron、原生）中的兼容性
```

## 路线图

- **v0**：文件夹配置 spell
- **vFuture**：GUI 配置界面

---

## Open Questions

### Q1: 技术栈
选项：
- **Python** — 开发快，库丰富（keyboard、pyperclip、PyQt/tkinter）
- **Tauri (Rust + Web)** — 轻量、性能好，可打包 exe
- **Electron** — 开发体验好，但较重

### Q2: 读取选中文本的方式
- **方案A（剪贴板）**：触发时模拟 `Ctrl+C`，从剪贴板读取。简单，但会短暂覆盖剪贴板内容。
- **方案B（Accessibility API）**：直接读取选中内容。更干净，但实现复杂。

**A2：采用方案A。** 方案B在 Electron 类 app（VS Code、Slack 等）中覆盖率不稳定，用户体验差。剪贴板覆盖问题可通过以下时序规避：
```
保存剪贴板 → 模拟 Ctrl+C / Cmd+C → 读取选中文本 → 执行 spell → 模拟 Ctrl+V / Cmd+V → 恢复剪贴板
```
方案B可作为未来增强项（支持的 app 优先用B，否则降级到A）。

### Q3: 变换型 spell 在无选中文本时的行为
- 报错提示用户？
- 以空字符串作为输入正常运行？
- 在 spell 元数据中声明是否需要输入？

### Q4: Spell 文件夹的内部结构
每个 spell 是一个文件夹，文件夹内如何组织？需要考虑：
- 入口文件叫什么？如何标识（固定名称 `run.py`？还是在元数据中指定）？
- 是否需要元数据文件（如 `spell.json`）来描述 spell 的名称、描述、图标等？
- 静态 spell（纯文本输出）怎么表示？单独一个 `output.txt`？还是也用脚本？
- 文件夹名称是否就是 spell 的显示名称？还是由元数据指定？
- 是否支持 spell 依赖其他文件（如配置、模型权重）放在同一文件夹内？
