# ADR 0001：跨平台桌面技术栈

- 状态：Accepted
- 日期：2026-08-08

## 背景

V1.0 使用 SwiftUI/AppKit，只能覆盖 macOS。产品现要求同时支持 macOS Intel、
macOS Apple Silicon、Windows x64 和 Ubuntu x64，且首期产物不使用商业代码签名。

## 决策

- 使用 Tauri 2 + TypeScript/React 构建共享桌面 UI；
- 使用 Rust workspace 实现共享领域与应用服务；
- 使用小型平台 Adapter 隔离系统代理、TUN、权限、生命周期和安装行为；
- macOS NetworkExtension 继续使用 Swift 原生 target，但 unsigned profile 不启用它；
- Windows/Linux 的 TUN 后端在 Phase 0 Spike 后分别形成 ADR，不提前锁定实现。

## 理由

- 相比维护三套 UI，共享 UI 和领域层显著减少重复代码；
- 相比 Electron，Tauri 使用系统 WebView，更符合当前资源目标；
- Rust 适合无 UI 的可测试共享层，并能生成三端原生二进制；
- 特权网络能力本来就受操作系统 API 约束，保留原生 Adapter 比强行统一更清晰。

## 后果

- macOS 外观不是纯 SwiftUI，需要通过设计系统和平台样式校准；
- 三个平台仍需要独立的权限、安装、系统代理和 TUN 集成测试；
- 未签名 macOS 构建无法把 NetworkExtension TUN 作为可用功能；
- Windows 未签名安装包会触发 SmartScreen 提示。
