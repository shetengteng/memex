# 已归档：原型 Vue 应用

> 状态：**已归档**（自 2026-06-07 起）
> 后续工作位置：`tauri-app/src/`

## 这是什么

这是 Memex 桌面应用的 UI 原型工程，最初由 Claude 用 Vue 3 + Tailwind + shadcn-vue 搭建，作为"完整桌面应用"重构方案的**视觉原型 / 交互草图**。

## 已经发生了什么

2026-06-07 完成了 **prototype-app → tauri-app 全量迁移**：

- 21 个 view 全部搬到 `tauri-app/src/views/`
- 全部 mock data 替换为真实 IPC 调用（`stores/memex.ts` reactive store）
- 与原 popup 应用的 IPC 兼容层、deep link、Ollama 引导、托盘行为全部接齐
- 详见：`design/20260607-01-Memex-popup替换为桌面应用-重构方案.md` 与 `20260607-02-Memex-popup转桌面-TODO.md`

## 你应该看哪里

- 当前生产代码：`tauri-app/src/`
- 当前测试套件：`tauri-app/src/**/*.test.ts` + `tauri-app/src-tauri/tests/`
- 历史决策记录：`design/20260607-01-*.md` / `design/20260607-02-*.md`

## 为什么不直接删除

保留这份原型作为：

1. **UI 决策溯源**：第一版的视觉风格、交互模式来源
2. **新人参考**：未接触过整个体系时，单独跑原型 (`npm install && npm run dev`) 比跑整个 Tauri 应用快
3. **未来 UI 迭代沙盒**：如果再有大的 UI 重构，仍可在这里迭代视觉，验证后整体替换

但**不要在这里继续开发新功能** —— 所有 functional 改动一律去 `tauri-app/`。
