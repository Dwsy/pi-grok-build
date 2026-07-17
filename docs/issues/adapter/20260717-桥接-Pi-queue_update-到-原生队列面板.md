---
id: "2026-07-17-桥接-pi-queue-update-到-原生队列面板"
title: "桥接 Pi queue_update 到 x.ai/queue/changed"
status: "completed"
created: "2026-07-17"
updated: "2026-07-17"
category: "adapter"
tags: ["workhub", "queue", "follow-up", "steer"]
---

# Issue: 桥接 Pi queue_update 到 x.ai/queue/changed

## Goal

消除 grok-pi mid-turn 消息「已入队 / AI 已处理 / UI 永不消队」与「Send now hover 无效」的断环，让原生 QueuePane 以 Pi 队列为权威镜像。

## 根因

| 层 | 行为 |
|---|---|
| Pager | turn 进行中走 `immediate_server_send`：optimistic 写入 `shared_queue`，等 `x.ai/queue/changed` 确认/出队 |
| Adapter | 收 ACP prompt → Pi `prompt` + `streamingBehavior`；`queue_update` **只**刷 status 文案 |
| Pi | `queue_update` 已带 `steering: string[]` / `followUp: string[]` 全文；交付时 `message_start` 会摘条并再发 `queue_update` |

`x.ai/*` 是上游 Grok Build ACP 扩展**命名空间**，不是 xAI 云产品依赖。本项目复用协议形状，但未接队列生命周期。

## 边界

1. Pi 是队列权威；adapter 只镜像，不做第二套 scheduler。
2. 不改 Pi 源码扩 RPC。
3. Pi RPC **无** `clearQueue` / 按 id 删除 / 编辑 / 重排；对应 `x.ai/queue/{remove,clear,edit,reorder}` 只能 rebroadcast 当前镜像（必要时 toast），不能假装成功。
4. `x.ai/queue/interject`：无单条删除时无法安全「只 promote 一条」；映射为对该条文案的 `steer`（与现有 `x.ai/interject` 一致），并 rebroadcast。followUp 条可能仍会被 Pi 再投递一次——记为已知边界。
5. mid-turn 默认 `followUp`；`sendNow: true` → `steer`（对齐 FEATURE_MATRIX）。

## 方案

### A. QueueMirror（adapter 内）

- 维护 `steering` / `followUp` 有序镜像与稳定 `id`。
- ACP mid-turn prompt 若带 `promptId`，按 `(text, lane)` 预留 id，使 optimistic echo **按 id** 命中。
- `queue_update` → 调和镜像 → 广播 `x.ai/queue/changed`：
  - `entries`: kind=`prompt`，position 连续
  - `runningPromptId`: 本轮从镜像消失的条目（Pi 交付摘条）
- 保留 status：`N steering` / `N follow-up`。

### B. prompt streamingBehavior

```text
!already_active          → None
meta.sendNow == true     → steer
meta.followUp == true    → followUp
default (mid-turn)       → followUp
```

### C. ext_notification

| 方法 | 行为 |
|---|---|
| `x.ai/queue/remove` | rebroadcast + toast 边界 |
| `x.ai/queue/clear` | rebroadcast + toast 边界 |
| `x.ai/queue/edit` | rebroadcast + toast 边界 |
| `x.ai/queue/reorder` | rebroadcast |
| `x.ai/queue/interject` | `steer(text)` + rebroadcast |

## 验收

1. mid-turn 发送后 QueuePane 出现行；Pi 开始处理该条后行消失（不再幽灵挂起）。
2. 确认后的行在 turn running 时渲染 `[Send now]`，hover 高亮。
3. 空 composer Enter / send-now 仍走现有 interject/steer 路径。
4. `cargo test -p pi-grok-adapter` 通过；含 QueueMirror 与 streamingBehavior 单测。
5. FEATURE_MATRIX Queue 行更新为「Pi queue_update 全文 → x.ai/queue/changed」。

## 非目标

- 不实现多 client 共享队列一致性（单机 Pi）。
- 不改 Pager source-identity 接缝（纯 adapter 广播 + 既有 handler）。
- 不在 Pi 源码增加 clearQueue RPC。

## 进度

- [x] 根因与协议事实
- [x] QueueMirror + queue_update 广播（`queue_bridge.rs`）
- [x] streamingBehavior 默认 followUp；`sendNow` → steer
- [x] ext_notification 队列方法（remove/clear/edit/reorder rebroadcast；interject → steer）
- [x] 测试：`cargo test -p pi-grok-adapter` → 44 passed（含 5 个 QueueMirror 单测）
- [x] FEATURE_MATRIX 已更新

## 实现要点

- `queue_bridge::QueueMirror`：预留 client `promptId`、调和 Pi 全文数组、消失项 → `runningPromptId`
- `queue_update` → `x.ai/queue/changed` + status
- `agent_settled` 清 `runningPromptId` 并 rebroadcast，避免 idle 后幽灵 running id
- **PromptResponse 回显 `promptId`**：mid-turn 1/2/3 与主 turn 同批 settle 时，Pager 只对 `current_prompt_id` 画 `Worked for`；无 meta 时会误 finish 出 3 个 `Worked for 0.0s`
