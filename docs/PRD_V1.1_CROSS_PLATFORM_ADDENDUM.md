# Magies Proxy PRD V1.1 — 跨平台与未签名发行增补

> 日期：2026-08-08  
> 状态：开发基线  
> 基础文档：`Magies_Proxy_PRD_V1.0.md`

## 1. 文档效力

本增补把产品范围从单一 macOS 客户端调整为桌面三端客户端。与 V1.0
第 3、4、9、10、19、26、38、39、44、47～50、54、55 节冲突的内容，
以本增补为准；未冲突的协议、领域模型、Core Adapter、路由、DNS、日志、
安全与隐私要求继续有效。

## 2. 目标平台

| 平台 | 最低版本 | CPU | V0.1 状态 |
|---|---|---|---|
| macOS | 13 | `x86_64` | P0，必须在真实 Intel 设备验证 |
| macOS | 13 | `aarch64` | P0 |
| Windows | 10/11 | `x86_64` | P0 |
| Linux | Ubuntu 22.04+ | `x86_64` | P0 |

iOS、Android、Windows ARM64 和 Linux ARM64 不属于 V0.1。

## 3. 技术架构

```text
Tauri 2 + TypeScript/React Desktop UI
                  │
             Tauri Commands
                  │
       Rust Application Services
  Profile / Core / Route / DNS / Monitor
                  │
        Platform Adapter Interface
          ┌───────┼────────┐
          │       │        │
       macOS   Windows   Linux
       Swift   Service   Service
       NE      TUN       TUN
          └───────┼────────┘
                  │
          Xray / sing-box
```

架构约束：

1. UI、领域模型、配置生成、订阅解析和 Core 生命周期只实现一次；
2. 系统代理、权限、TUN、休眠/网络恢复和开机启动通过平台 Adapter 隔离；
3. Rust 领域层不得依赖 Tauri，以便单元测试和未来 CLI/服务复用；
4. UI 不直接读写 Xray 或 sing-box JSON；
5. 平台特权进程只暴露最小 IPC，不接收任意命令或路径；
6. 各平台使用相同数据库 schema 和统一领域模型；
7. TUN 的 Windows/Linux 具体后端必须经 Phase 0 Spike 后锁定，不在文档阶段猜测。

Tauri 官方为 Linux、macOS 和 Windows 提供桌面开发前置要求，并支持这些平台的
安装包分发；Windows 使用 WebView2，macOS/Linux 使用系统 WebView。参考：

- https://v2.tauri.app/start/prerequisites/
- https://v2.tauri.app/distribute/

## 4. 工程结构

```text
MgClash/
├── apps/
│   └── desktop/                 # Tauri + React UI
├── crates/
│   ├── magies-domain/           # 领域模型
│   ├── magies-platform/         # 平台/架构与能力矩阵
│   ├── magies-core-runtime/     # Core 进程与状态机
│   ├── magies-profiles/         # 导入、订阅、配置生成
│   ├── magies-routing/
│   ├── magies-storage/
│   └── magies-monitoring/
├── platform/
│   ├── macos/                   # Swift + NetworkExtension/SystemConfiguration
│   ├── windows/                 # Windows service/TUN adapter
│   └── linux/                   # system service/TUN adapter
├── resources/
├── scripts/
└── tests/
```

只在任务实际开始时创建对应目录和 crate，不预先生成空模块。

## 5. 未签名发行策略

当前所有平台均不使用商业代码签名：

| 平台 | 主要产物 | 限制 |
|---|---|---|
| macOS | `.app`/压缩包，必要时仅 ad-hoc 签名 | 不公证；Gatekeeper 会提示；无有效 Network Extension entitlement 时禁用 TUN |
| Windows | portable ZIP，后续可附未签名安装包 | SmartScreen 会提示；内核驱动必须使用上游有效签名版本 |
| Linux | tarball，后续 `.deb`/AppImage | 暂不提供仓库或 GPG 签名 |

“未签名”指不使用 Apple Developer ID、Microsoft Authenticode 或 Linux 包仓库签名。
若 macOS 工具链要求 ad-hoc 签名，它不等同于开发者身份签名或公证。

Apple 文档明确说明 `NEPacketTunnelProvider` 需要
`com.apple.developer.networking.networkextension` entitlement。因此 macOS 未签名
构建的 V0.1 验收范围调整为本地 HTTP/SOCKS 与 System Proxy；TUN UI 必须显示
`UnavailableInUnsignedBuild`，不得尝试启动后再静默失败。

参考：https://developer.apple.com/documentation/networkextension/nepackettunnelprovider

## 6. 构建能力矩阵

| 能力 | macOS unsigned | Windows unsigned | Linux unsigned |
|---|---:|---:|---:|
| 节点/订阅/规则 | 是 | 是 | 是 |
| Xray/sing-box | 是 | 是 | 是 |
| HTTP/SOCKS | 是 | 是 | 是 |
| System Proxy | 是 | 是 | 是 |
| TUN | 否 | Phase 0 验证 | Phase 0 验证 |
| 自动恢复系统代理 | 是 | 是 | 是 |

macOS TUN 代码仍保留独立适配层和测试入口，但不进入未签名发行包的功能承诺。

## 7. CI 矩阵

每个 Pull Request 必须运行：

| Runner | 验证目标 |
|---|---|
| `macos-15-intel` | macOS Intel 单元测试、静态检查、构建 |
| `macos-15` | macOS Apple Silicon 单元测试、静态检查、构建 |
| `windows-2022` | Windows x64 单元测试、静态检查、构建 |
| `ubuntu-22.04` | Linux x64 单元测试、静态检查、构建 |

GitHub 当前将 `macos-15-intel` 列为 x64 runner。该托管 Intel runner 已公告只保障到
2027 年 8 月；在此之前必须准备真实 Intel Mac 或自托管 runner 作为后备。

参考：

- https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- https://github.com/actions/runner-images/issues/13045

## 8. 调整后的开发顺序

### Phase 0A — 跨平台基础

- [ ] CP01 独立仓库、workspace、格式化与静态检查；
- [ ] CP02 目标平台/CPU 识别与 typed error；
- [ ] CP03 三端 CI 矩阵，Intel job 不允许降级为 arm64；
- [ ] CP04 unsigned 构建能力开关；
- [ ] CP05 Tauri 最小窗口与 Rust command smoke test。

退出条件：四个目标组合都能编译共享层，平台识别测试通过，Intel CI 独立运行。

### Phase 0B — 平台能力 Spike

- macOS Intel：Xray/sing-box 启停、本地代理、System Proxy 保存/恢复；
- macOS：验证 unsigned 构建会显式禁用 NetworkExtension TUN；
- Windows：Core 启停、System Proxy、管理员服务与 TUN 可行性；
- Linux：Core 启停、桌面代理设置、polkit/system service 与 TUN 可行性。

退出条件：每个平台都有书面结论、可重复 smoke test 和已锁定 Adapter 边界。

### Phase 1 — 共享 Core + Node

沿用 V1.0 Phase 1，但实现于共享 Rust crates，所有 parser/config generator 测试在
三端运行。

### Phase 2 — Subscription + System Proxy

共享订阅事务；分别实现三端 System Proxy Adapter 和恢复测试。

### Phase 3 — TUN + DNS + Route

Windows/Linux 按 Phase 0B 的技术结论实现。macOS unsigned 版本保持 TUN 不可用；
未来签名版作为独立发行 profile 验收。

### Phase 4 — V0.1 稳定化与未签名发布

- macOS Intel/Apple Silicon unsigned artifact；
- Windows x64 portable ZIP 与未签名 installer；
- Ubuntu x64 tarball；
- 睡眠/网络变化/Core crash/System Proxy 恢复；
- 每个平台 72 小时稳定性测试。

## 9. Definition of Done 增补

任一 P0 功能还必须满足：

1. 共享逻辑单元测试覆盖率不低于 80%；
2. 平台特定功能至少有 Adapter contract test 和对应系统 integration test；
3. 不支持的能力在启动前返回 typed error，UI 不显示为可用；
4. macOS Intel 不得只靠交叉编译声明支持，必须有 Intel runner 或真机结果；
5. release 文件名包含 OS、CPU、版本和 `unsigned`；
6. 未签名风险在下载页和首次启动说明中可见。
