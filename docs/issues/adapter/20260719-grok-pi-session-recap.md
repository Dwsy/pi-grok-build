---
id: "2026-07-19-grok-pi-session-recap"
title: "grok-pi 支持 session recap（F2 开关/模型 + 系统语言）"
status: "completed"
created: "2026-07-19"
updated: "2026-07-19"
category: "adapter"
tags: ["workhub", "recap", "settings", "extension"]
---

# Issue: grok-pi 支持 session recap

## Goal

让 `grok-pi` 具备与原生 Grok 一致的 `/recap` + 自动 return-from-away recap，并在 F2 设置中可开关、可选模型；recap 正文使用当前系统语言。

## 边界

1. **Grok Pager 是唯一 TUI** — 复用 `Action::SendRecap` / `x.ai/recap` / `SessionRecap` scrollback。
2. **Pi 是唯一 agent core** — 不改 Pi 源码；用 NamedTempFile extension + `complete()` 做 display-only 侧调用。
3. **adapter headless** — 只做 RPC/ext 映射，不渲染。
4. recap **不得写入** Pi 会话 LLM 历史（custom message `display:false` 仅桥接）。

## 方案

| 层 | 行为 |
|---|---|
| composition | 注入 `__pi_grok_recap` extension（`complete` + `sendMessage` custom） |
| adapter | `initialize.meta.sessionRecap=true`；`x.ai/recap` → bridge command；custom `pi-grok-recap/v1` → `SessionRecap` / `SessionRecapUnavailable` |
| pager | F2：`session_recap` 开关 + `recap_model`；`SendRecap` 带 model；external 也启用 recap poll |
| 语言 | adapter 读 `LANG`/`LC_ALL`，写入 instruction「用该语言输出」 |

## 验收

- [x] `/recap` 路径：`Action::SendRecap` → `x.ai/recap` → `__pi_grok_recap` → `SessionRecap`
- [x] F2：`session_recap` 开关（auto）+ `recap_model`（空 = 会话模型）
- [x] 输出语言：adapter 读 `LC_ALL`/`LC_MESSAGES`/`LANG` 写入 instruction
- [x] `cargo test -p pi-grok-adapter` PASS（62）
- [x] `cargo check -p xai-grok-pager-bin --bin grok-pi` PASS
- [x] `cargo test -p xai-grok-pager-bin --bin grok-pi recap_extension` PASS
- [x] 修复 external 路径 `session_recap_available` 默认 false 导致 `/recap` 隐藏
- [ ] 手测：冷启动可见 `/recap`、F2 关 auto、换模型、中文 locale

## 非目标

- 不实现 Grok shell 的 recap artifact 落盘
- 不改 Pi RPC 协议
