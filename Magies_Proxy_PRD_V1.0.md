---
title: "Magies Proxy 产品需求与技术方案 PRD"
author: "Magies’s Dad"
date: "2026-08-08"
lang: zh-CN
---

# Magies Proxy 产品需求与技术方案 PRD

> **文档版本：** V1.0  
> **项目代号：** Magies Proxy  
> **首发平台：** macOS Intel x86_64  
> **后续平台：** macOS Apple Silicon arm64 / Universal Binary  
> **产品目标：** 先达到 v2rayN macOS 的主力代理能力，再逐步实现 Shadowrocket 风格的策略、脚本、Rewrite、MITM 与 Module 能力  
> **技术路线：** Swift + SwiftUI/AppKit + NetworkExtension + Xray Core + sing-box + SQLite + Keychain + JavaScriptCore  
> **文档状态：** 可立项 / 可拆解开发  
> **技术核实日期：** 2026-08-08

---

# 1. 文档控制

## 1.1 版本记录

| 版本 | 日期 | 状态 | 说明 |
|---|---|---|---|
| V0.1 | 2026-08-08 | 草案 | 初始产品方向：macOS 原生代理客户端 |
| V1.0 | 2026-08-08 | 基线 | 补齐功能、架构、数据模型、接口、测试、验收与路线图 |

## 1.2 文档用途

本文档用于：

1. 统一 Magies Proxy 的产品目标与边界；
2. 作为 Swift/macOS 客户端开发的技术基线；
3. 作为 Codex、Claude Code、Grok Build 或人工开发者的任务拆解依据；
4. 作为 V0.1、V0.5、V1.0 各阶段验收依据；
5. 约束后续功能扩展，避免 UI、代理核心、路由、DNS、模块系统相互耦合。

## 1.3 核心结论

Magies Proxy 不以“第一版 100% 复制 Shadowrocket”为目标。

产品路线固定为：

```text
第一阶段：可替代 v2rayN
        ↓
第二阶段：形成更原生的 macOS 使用体验
        ↓
第三阶段：策略组、连接分析、Rule Provider
        ↓
第四阶段：Module / Rewrite / Script
        ↓
第五阶段：MITM / Binary Body / 高级模块
```

第一阶段唯一核心验收标准：

> **卸载 v2rayN 后，Magies Proxy 仍可独立承担日常 macOS 代理工作。**

---

# 2. 产品背景

macOS 上成熟代理客户端大体可以分为两类：

- **Core 管理型客户端**：强调协议覆盖、订阅、节点、路由和多核心管理；
- **网络工具型客户端**：在代理基础上增加策略组、请求重写、脚本、MITM、模块等高级能力。

Magies Proxy 计划把两者的优势组合起来，但采用分阶段实现策略：

- V0.1 先解决“稳定可用”；
- V0.5 解决“好用、可观测、可自动选择”；
- V1.0 再解决“可编程、可扩展”。

截至本 PRD 技术核实日期：

- Apple `NetworkExtension` 提供 `NEPacketTunnelProvider`，可用于构建 packet tunnel/VPN 类型客户端 [R1][R2]；
- v2rayN 官方文档仍提供 macOS x64 发行说明，说明 Intel x86_64 仍有现实参考实现 [R3]；
- Xray 官方发布工作流包含 `darwin/amd64` 构建目标 [R4]；
- sing-box 提供 Apple/macOS 客户端、TUN 与 macOS 进程匹配相关能力 [R5][R6]。

因此，**Intel Mac 作为首发目标具有可行性**，但必须把 Intel Core 构建可用性纳入持续集成检查。

---

# 3. 产品定位

## 3.1 一句话定位

> **一个原生 macOS 代理客户端，优先支持 Intel Mac，以 v2rayN 级代理能力为底座，逐步加入 Shadowrocket 风格的高级网络工具能力。**

## 3.2 目标用户

### P0 用户

- 在 macOS 上使用 VLESS / VMess / Trojan / Shadowsocks / Hysteria2 的用户；
- 需要 TUN 全局接管的用户；
- 需要订阅、测速、路由、DNS、日志的用户；
- Intel Mac 用户；
- 希望界面比传统跨平台桌面客户端更符合 macOS 使用习惯的用户。

### P1 用户

- 需要策略组、自动测速、故障切换的高级用户；
- 需要按域名、IP、进程进行路由的用户；
- 需要查看实时连接与规则命中的用户。

### P2 用户

- 需要 Rewrite、Script、MITM、Module 的高级网络工具用户；
- 需要在本地调试 HTTP/HTTPS 流量或自定义脚本的开发者。

## 3.3 产品差异点

1. Intel x86_64 作为正式支持目标，而不是“能跑就算”；
2. UI 原生 SwiftUI/AppKit，不以 v2rayN 界面为模板；
3. Xray + sing-box 采用统一抽象，不让 UI 依赖某个 Core 的 JSON；
4. 第一阶段完整代理能力与第二阶段模块能力解耦；
5. Module 系统作为插件层，不把 WLOC 或某个具体模块写死在主程序；
6. 默认 Local First，不上传用户节点、订阅、DNS、流量历史。

---

# 4. 目标与非目标

## 4.1 V0.1 产品目标

V0.1 必须完成：

- macOS Intel x86_64 可安装、可运行；
- 节点创建、编辑、删除、排序、分组；
- 订阅添加、更新、自动更新；
- 分享链接/剪贴板导入；
- Xray Core；
- sing-box Core；
- System Proxy；
- TUN；
- DNS；
- Rule 路由；
- 延迟测试、URL Test；
- 实时速率、基础流量统计；
- Core/App 日志；
- 菜单栏快捷控制；
- 睡眠唤醒和网络切换后可恢复；
- 崩溃时尽可能恢复系统代理状态。

## 4.2 V0.5 产品目标

增加：

- SELECT / URL-TEST / FALLBACK / LOAD-BALANCE；
- Rule Provider；
- Connections；
- 进程识别；
- 规则命中显示；
- 节点健康度；
- 自动故障切换；
- 更细粒度的 DNS 策略；
- Core/规则独立更新机制。

## 4.3 V1.0 产品目标

增加：

- Module Parser；
- Rewrite Engine；
- JavaScriptCore Script Engine；
- Persistent Store；
- MITM 证书管理；
- HTTP Request/Response Rewrite；
- Binary Body 支持；
- 至少运行一个真实高级 Module 作为系统验证。

## 4.4 明确非目标

V0.1 不做：

- Windows/Linux/iOS/Android；
- 账号系统；
- 云同步；
- 模块商店；
- 自研 VLESS/VMess/Trojan/QUIC 协议实现；
- 自研完整 TCP/IP 协议栈；
- 100% 兼容 Shadowrocket/Surge/Loon/Quantumult X 的所有脚本 API；
- 默认全域名 MITM；
- 未经用户确认的证书安装。

---

# 5. 成功指标

## 5.1 V0.1 成功指标

| 指标 | 目标 |
|---|---|
| Intel 启动成功率 | >= 99%（测试设备范围内） |
| 连接成功率 | 正确配置节点下 >= 99% |
| 节点切换 | 目标 <= 2 秒，失败时明确提示 |
| 冷启动到可操作 | 目标 <= 3 秒 |
| 空闲 App CPU | 目标 < 2% |
| 空闲 App 内存 | 目标 < 200 MB，不含 Core |
| Sleep/Wake | 50 次循环无持续失联 |
| 网络切换 | Wi-Fi/热点/有线切换后自动恢复 |
| System Proxy 残留 | 正常退出不残留；异常退出提供恢复机制 |
| 连续运行 | 72 小时稳定性测试通过 |

## 5.2 产品级成功定义

- V0.1：**可替代 v2rayN 作为主力客户端**；
- V0.5：**在 macOS 原生体验和连接可观测性上明显优于基础 Core 管理客户端**；
- V1.0：**具备可扩展脚本与模块能力，开始进入 Shadowrocket 类网络工具定位**。

---

# 6. 用户场景与用户故事

## 6.1 首次使用

**作为用户，我希望：**

1. 安装 App；
2. 完成必要的 VPN/Network Extension 授权；
3. 粘贴订阅 URL；
4. 更新节点；
5. 选择一个节点；
6. 点击连接；
7. 浏览器和其他应用开始按规则走代理。

**验收条件：** 首次使用主流程不要求用户手写 JSON。

## 6.2 日常切换

**作为用户，我希望：**

- 不打开主窗口即可从菜单栏切换节点；
- 快速切换 Global / Rule / Direct；
- 查看当前节点、延迟和实时速率。

## 6.3 故障恢复

**作为用户，我希望：**

- 从 Wi-Fi 切换到 iPhone 热点后不用手工重启 App；
- Mac 睡眠再唤醒后代理自动恢复；
- Core 异常退出时 App 能够检测并恢复，或明确告知失败原因。

## 6.4 高级路由

**作为高级用户，我希望：**

- Apple、局域网等流量直连；
- Google/OpenAI 等走代理；
- 中国大陆 IP 直连；
- 某个进程固定走指定策略组。

## 6.5 模块扩展

**作为高级用户，我希望：**

- 通过 URL 或文件导入模块；
- 查看模块需要的 MITM 域名和脚本权限；
- 单独启用/关闭模块；
- 查看脚本执行日志；
- 删除模块后不影响代理主功能。

---

# 7. 功能范围与优先级

| 模块 | V0.1 | V0.5 | V1.0 | 优先级 |
|---|---:|---:|---:|---|
| Intel x86_64 | ✅ | ✅ | ✅ | P0 |
| Xray | ✅ | ✅ | ✅ | P0 |
| sing-box | ✅ | ✅ | ✅ | P0 |
| 节点管理 | ✅ | ✅ | ✅ | P0 |
| 订阅 | ✅ | ✅ | ✅ | P0 |
| System Proxy | ✅ | ✅ | ✅ | P0 |
| TUN | ✅ | ✅ | ✅ | P0 |
| DNS | ✅ | ✅ | ✅ | P0 |
| Rule | ✅ | ✅ | ✅ | P0 |
| 测速 | ✅ | ✅ | ✅ | P0 |
| 日志 | ✅ | ✅ | ✅ | P0 |
| 菜单栏 | ✅ | ✅ | ✅ | P0 |
| 流量统计 | 基础 | 完整 | 完整 | P1 |
| Connections | - | ✅ | ✅ | P1 |
| 策略组 | - | ✅ | ✅ | P1 |
| Rule Provider | - | ✅ | ✅ | P1 |
| 自动故障切换 | - | ✅ | ✅ | P1 |
| Module | - | - | ✅ | P2 |
| Rewrite | - | - | ✅ | P2 |
| Script | - | - | ✅ | P2 |
| MITM | - | - | ✅ | P2 |

---

# 8. 协议支持

## 8.1 P0 协议

- VLESS；
- VMess；
- Trojan；
- Shadowsocks；
- Hysteria2。

## 8.2 P1 协议

- SOCKS5；
- HTTP Proxy；
- TUIC。

## 8.3 P2 协议

- WireGuard；
- 后续由 Core 原生能力决定的其他协议。

## 8.4 原则

Magies Proxy 不直接实现协议本身，而由 Xray/sing-box 负责协议层。

UI 只保存统一的 `ProxyNode` 领域模型，再由 Config Generator 生成具体 Core 配置。

---

# 9. 总体技术架构

```text
┌───────────────────────────────────────────────────────────┐
│                     Magies Proxy App                      │
│                 SwiftUI + AppKit + MenuBar                │
└───────────────────────────┬───────────────────────────────┘
                            │
                    Application Services
                            │
       ┌──────────────┬─────┼─────┬──────────────┐
       │              │           │              │
 ProfileManager   CoreManager  RouteEngine   DNSManager
       │              │           │              │
       │         ┌────┴────┐      │              │
       │         │         │      │              │
       │       Xray     sing-box  │              │
       │         │         │      │              │
       └─────────┴────┬────┴──────┴──────────────┘
                      │
                 TunnelService
                      │
             NetworkExtension Target
                      │
            NEPacketTunnelProvider
                      │
                     TUN
                      │
                    macOS

V1.0 Extension Layer:

        ModuleManager
             │
      ┌──────┼──────────┐
      │      │          │
   Rewrite  Script     MITM
             │          │
     JavaScriptCore   CA/TLS
```

## 9.1 架构原则

1. **UI 与 Core 解耦**：UI 不直接编辑 Xray/sing-box JSON；
2. **Core 可替换**：任何 Core 都通过统一 Adapter 接入；
3. **TUN 与主 UI 进程解耦**：Network Extension 独立 Target；
4. **配置单一事实源**：数据库领域模型为 Source of Truth；
5. **生成配置可重建**：Core JSON 属于生成物，不作为唯一数据源；
6. **模块隔离**：Module 不可直接修改主程序数据库；
7. **敏感数据进 Keychain**：订阅 token、节点密码、私钥等不明文存储。

---

# 10. Xcode 工程结构

```text
MagiesProxy/
├── Apps/
│   ├── MagiesProxyApp/
│   └── MagiesPacketTunnel/
│
├── Packages/
│   ├── MagiesDomain/
│   ├── MagiesStorage/
│   ├── MagiesProfiles/
│   ├── MagiesCoreRuntime/
│   ├── MagiesRouting/
│   ├── MagiesDNS/
│   ├── MagiesMonitoring/
│   ├── MagiesModules/
│   └── MagiesSharedUI/
│
├── Resources/
│   ├── Geo/
│   ├── Templates/
│   └── DefaultRules/
│
├── Vendor/
│   ├── xray/
│   └── sing-box/
│
├── Scripts/
│   ├── build-core.sh
│   ├── verify-core.sh
│   ├── package-intel.sh
│   └── package-universal.sh
│
└── Tests/
    ├── UnitTests/
    ├── IntegrationTests/
    └── UITests/
```

建议优先使用 Swift Package 拆分内部模块，降低 Xcode Target 之间的耦合。

---

# 11. 领域模型

## 11.1 ProxyNode

```swift
struct ProxyNode: Identifiable, Codable, Sendable {
    let id: UUID
    var name: String
    var protocolType: ProxyProtocol
    var server: String
    var port: Int
    var credentialRef: String?
    var transport: TransportConfig?
    var tls: TLSConfig?
    var udpEnabled: Bool
    var subscriptionID: UUID?
    var groupID: UUID?
    var latencyMS: Int?
    var lastTestedAt: Date?
    var enabled: Bool
}
```

## 11.2 Subscription

```swift
struct Subscription: Identifiable, Codable, Sendable {
    let id: UUID
    var name: String
    var urlSecretRef: String
    var updateIntervalMinutes: Int
    var autoUpdate: Bool
    var lastUpdatedAt: Date?
    var etag: String?
    var lastModified: String?
    var enabled: Bool
}
```

## 11.3 RoutingRule

```swift
struct RoutingRule: Identifiable, Codable, Sendable {
    let id: UUID
    var type: RuleType
    var value: String
    var outbound: String
    var priority: Int
    var enabled: Bool
}
```

## 11.4 CoreProfile

```swift
struct CoreProfile: Codable, Sendable {
    var coreType: CoreType
    var selectedNodeID: UUID?
    var listenHTTP: Int
    var listenSOCKS: Int
    var routingMode: RoutingMode
    var dnsProfileID: UUID?
    var tunEnabled: Bool
}
```

---

# 12. 数据库设计

推荐 SQLite。V0.1 可使用 GRDB 或轻量封装，避免业务层直接拼 SQL。

## 12.1 主要表

### `nodes`

| 字段 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | UUID |
| name | TEXT | 节点名称 |
| protocol | TEXT | vless/vmess/trojan/... |
| server | TEXT | 主机 |
| port | INTEGER | 端口 |
| credential_ref | TEXT | Keychain 引用 |
| transport_json | TEXT | 传输配置 |
| tls_json | TEXT | TLS 配置 |
| subscription_id | TEXT | 来源订阅 |
| group_id | TEXT | 分组 |
| latency_ms | INTEGER | 最近延迟 |
| enabled | INTEGER | 是否启用 |
| created_at | INTEGER | 时间戳 |
| updated_at | INTEGER | 时间戳 |

### `subscriptions`

| 字段 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | UUID |
| name | TEXT | 名称 |
| url_secret_ref | TEXT | Keychain 引用 |
| auto_update | INTEGER | 是否自动更新 |
| update_interval | INTEGER | 分钟 |
| etag | TEXT | HTTP 缓存 |
| last_modified | TEXT | HTTP 缓存 |
| last_updated_at | INTEGER | 时间戳 |
| enabled | INTEGER | 是否启用 |

### `routing_rules`

| 字段 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | UUID |
| rule_type | TEXT | domain/ip/process/... |
| value | TEXT | 匹配值 |
| outbound | TEXT | 目标策略 |
| priority | INTEGER | 优先级 |
| enabled | INTEGER | 是否启用 |

### `dns_profiles`

保存 DNS 类型、server、DoH/DoT、bootstrap、规则映射等。

### `app_settings`

保存非敏感偏好：主题、菜单栏、启动行为、日志等级、窗口状态等。

### `traffic_daily`

保存按日聚合流量；V0.1 不默认保存完整域名访问历史。

## 12.2 Keychain 数据

以下内容禁止明文进入 SQLite：

- 节点 UUID/密码等认证数据（是否全部加密可按协议字段细分）；
- Hysteria2/TUIC 密码；
- WireGuard private key；
- 订阅 URL 中的 token；
- MITM CA 私钥；
- 用户未来定义的模块 secret。

Apple Keychain 是 macOS 上管理密码、密钥、证书等敏感数据的系统能力 [R8][R9]。

---

# 13. Core Runtime 设计

## 13.1 Core 抽象

```swift
protocol ProxyCoreAdapter: Sendable {
    var kind: CoreType { get }

    func validate(profile: RuntimeProfile) async throws
    func generateConfiguration(profile: RuntimeProfile) async throws -> URL
    func start(configURL: URL) async throws -> CoreSession
    func stop(session: CoreSession) async
    func health(session: CoreSession) async -> CoreHealth
}
```

## 13.2 CoreManager 职责

- 检查 Core 文件是否存在；
- 校验 CPU 架构；
- 校验版本和 hash；
- 生成运行目录；
- 生成配置；
- 启动/停止进程；
- 捕获 stdout/stderr；
- 解析关键错误；
- 维护状态机；
- Crash Recovery；
- 支持 Core 版本固定与回滚。

## 13.3 Core 状态机

```text
stopped
   │ start
   ▼
preparing
   │ config valid
   ▼
starting
   │ health ok
   ▼
running
   │
   ├── user stop ─────> stopping ─────> stopped
   │
   ├── core crash ────> recovering
   │                        │
   │                   retry success
   │                        ▼
   └──────────────────── running

recovering retry >= threshold
   ↓
failed
```

V0.1 自动恢复最多连续 3 次，之后进入 `failed`，避免无限重启。

---

# 14. Core 选择策略

## 14.1 默认策略

- 优先把“通用协议 + TUN + DNS + 路由”能力交给 sing-box；
- Xray 用于 Xray 特有或表现更优的协议/传输配置；
- 用户可手工选择 Core；
- 自动模式由节点协议和能力矩阵决定。

## 14.2 能力矩阵

必须建立内部 `CoreCapabilityMatrix`，例如：

```swift
struct CoreCapability {
    let protocolType: ProxyProtocol
    let supportsTUN: Bool
    let supportsUDP: Bool
    let supportsReality: Bool
    let supportsMux: Bool
    let supportedArchitectures: Set<CPUArchitecture>
}
```

禁止在 UI 中散落 `if core == xray` 的判断。

---

# 15. 节点导入与解析

## 15.1 分享链接

V0.1 支持：

```text
vless://
vmess://
trojan://
ss://
hysteria2://
hy2://
tuic://
```

## 15.2 导入入口

- Clipboard；
- 文本输入；
- 文件；
- 订阅 URL；
- QR Code（P1，可使用摄像头或图片识别）。

## 15.3 Parser 架构

```swift
protocol NodeURIParser {
    func canParse(_ value: String) -> Bool
    func parse(_ value: String) throws -> ProxyNode
}
```

每种协议独立 Parser，不写成单个超大 `switch`。

## 15.4 去重

推荐节点指纹：

```text
protocol + server + port + credential identity + transport + tls identity
```

订阅更新时：

- 已存在：更新属性；
- 新增：插入；
- 订阅删除的节点：按策略移除或标记失效；
- 手工修改的备注可保留。

---

# 16. 订阅系统

## 16.1 功能

- 新增；
- 编辑；
- 删除；
- 手动更新；
- 自动更新；
- 批量更新；
- ETag / Last-Modified；
- 请求超时；
- 更新失败保留旧节点；
- 显示最后更新时间与错误。

## 16.2 更新事务

订阅更新必须事务化：

```text
下载
 ↓
解析到临时集合
 ↓
全部校验
 ↓
生成 diff
 ↓
数据库事务提交
 ↓
刷新 UI
```

禁止边解析边删除旧数据，避免网络/解析失败导致订阅被清空。

---

# 17. 延迟测试与健康检查

## 17.1 测试类型

- TCP Connect；
- HTTP URL Test；
- Real Connection Test；
- Core Health Check。

默认 URL 可使用用户可配置的 204 地址。

## 17.2 并发控制

节点批量测试：

- 最大并发默认 8；
- 每节点超时默认 5 秒；
- 支持取消；
- UI 显示测试中/成功/超时/失败。

## 17.3 延迟不等同吞吐

UI 不使用“延迟最低 = 一定最快”的绝对表述。后续可加入小流量吞吐测试，但默认关闭以避免浪费流量。

---

# 18. System Proxy

## 18.1 本地端口

默认建议：

```text
SOCKS5 127.0.0.1:10808
HTTP   127.0.0.1:10809
```

端口必须可配置，并检测冲突。

## 18.2 系统代理状态管理

启动代理前保存：

- 当前网络服务；
- 原 HTTP Proxy；
- 原 HTTPS Proxy；
- 原 SOCKS Proxy；
- PAC 配置（如有）。

退出时恢复原状态，而不是简单全部关闭。

## 18.3 异常恢复

App 每次启动时检查：

```text
上次是否处于 system proxy active
+ 当前代理是否仍指向 Magies 端口
+ Core 是否不存在
```

若发现残留，提供：

> “检测到上次异常退出留下的系统代理，是否恢复网络设置？”

---

# 19. TUN / NetworkExtension

## 19.1 技术路径

使用独立 Network Extension Target：

```text
MagiesProxy.app
   │
   ├── UI / Core / Profile
   │
   └── MagiesPacketTunnel.appex
            │
     NEPacketTunnelProvider
            │
            TUN
```

Apple 官方 `NEPacketTunnelProvider` 用于实现 packet tunnel provider，并要求相应 Network Extension entitlement [R1][R2][R7]。

## 19.2 V0.1 TUN 功能

- Enable/Disable；
- IPv4；
- IPv6（可独立开关）；
- MTU；
- Auto Route；
- DNS 接管；
- Include/Exclude Route；
- 当前状态；
- Extension 错误回传主 App。

## 19.3 TUN IPC

主 App 与 PacketTunnel 之间只传：

- Profile ID；
- 启停命令；
- 状态；
- 简化统计；
- 错误信息。

不要把所有 Node/Rule 对象通过 IPC 全量传输。Extension 从共享 App Group 的“已生成运行配置”读取。

## 19.4 最早期技术验证

**立项后第一个技术 Spike 必须是：**

> 在 Intel Mac 上跑通一个最小 `NEPacketTunnelProvider` Demo，并验证签名、entitlement、启动、停止和网络恢复。

该验证必须早于完整 UI 开发。

---

# 20. DNS 设计

## 20.1 V0.1 能力

- System DNS；
- Plain DNS；
- DoH；
- DoT；
- IPv4/IPv6 策略；
- 基础规则分流；
- TUN 下 DNS Hijack；
- FakeIP（由 Core 能力实现）。

## 20.2 DNS Profile

```swift
struct DNSProfile: Identifiable, Codable {
    let id: UUID
    var name: String
    var mode: DNSMode
    var servers: [DNSServer]
    var rules: [DNSRule]
    var fakeIPEnabled: Bool
    var ipv6Enabled: Bool
}
```

## 20.3 DNS 安全

- DoH hostname 需要 bootstrap 处理；
- 禁止形成 DNS 递归回环；
- 配置生成器检测“代理 DNS 服务器本身需要通过自己解析”的明显循环；
- DNS 日志默认只保存在内存或短期日志文件，不进入长期用户画像。

---

# 21. 路由系统

## 21.1 基础模式

```text
Global
Rule
Direct
```

## 21.2 Rule 类型

V0.1：

- DOMAIN；
- DOMAIN-SUFFIX；
- DOMAIN-KEYWORD；
- IP-CIDR；
- IP-CIDR6；
- GEOIP；
- GEOSITE；
- PORT；
- NETWORK；
- FINAL。

V0.5：

- PROCESS-NAME；
- PROCESS-PATH；
- Rule Provider；
- 网络类型/接口条件（Core 支持时）。

sing-box 文档目前明确列出 macOS 的 `process_name` / `process_path` 规则支持 [R6]，具体是否可用于当前 TUN/部署形态必须通过 Spike 验证，不直接假设所有模式均可用。

## 21.3 规则执行顺序

```text
1. 本地保留/安全规则
2. 用户显式规则
3. Rule Provider
4. Geo 规则
5. FINAL
```

规则编辑界面必须显示排序，禁止用户误以为规则是无序集合。

---

# 22. 策略组（V0.5）

## 22.1 类型

- SELECT；
- URL-TEST；
- FALLBACK；
- LOAD-BALANCE。

## 22.2 示例

```text
Proxy
├── Auto
├── US
├── JP
├── SG
└── DIRECT

Auto (URL-TEST)
├── US-SJ-01  32 ms
├── JP-TK-01  71 ms
└── SG-01     84 ms
```

## 22.3 URL-Test 防抖

- 切换阈值默认不只比较 1 ms；
- 最短保持时间；
- 连续失败次数；
- 避免节点在相近延迟之间频繁抖动。

---

# 23. Connections（V0.5）

## 23.1 字段

```text
Process
Domain
Destination IP
Destination Port
Network
Matched Rule
Outbound/Policy
Upload
Download
Duration
State
```

## 23.2 操作

- 搜索；
- 按进程过滤；
- 按策略过滤；
- 按规则过滤；
- Kill Connection（Core 支持时）；
- 复制域名/IP；
- 快速生成规则草稿。

## 23.3 隐私

默认不长期保存完整 Connections 历史。

如用户开启历史记录，应明确提示：该功能会记录访问目标元数据。

---

# 24. 流量统计

## 24.1 首页

显示：

- 当前下载速率；
- 当前上传速率；
- 当前节点；
- 当前延迟；
- 今日流量。

## 24.2 统计粒度

V0.1：

- 实时；
- 今日；
- 本月；
- 总计。

V0.5：

- 按节点；
- 按策略组；
- 按进程（能力允许时）。

## 24.3 持久化

每分钟或退出时做聚合，不按每个 packet 写 SQLite。

---

# 25. 日志与可观测性

## 25.1 日志分类

- App；
- Core；
- Tunnel；
- DNS；
- Route；
- Subscription；
- Module（V1.0）；
- MITM（V1.0）。

## 25.2 日志等级

```text
Debug
Info
Warning
Error
```

## 25.3 脱敏

日志输出前自动处理：

- subscription token；
- password；
- UUID/credential（可配置部分遮盖）；
- private key；
- Authorization Header。

“导出诊断包”必须先脱敏。

---

# 26. 菜单栏设计

推荐结构：

```text
● Magies Proxy

Connected
US-SJ-01        32 ms
↓ 18.4 MB/s     ↑ 3.2 MB/s

──────────────
Rule ✓
Global
Direct
──────────────
Auto
US-SJ-01
JP-TK-01
SG-01
──────────────
Open Magies Proxy
Disconnect
Quit
```

菜单栏要求：

- 不依赖主窗口打开；
- 节点切换后显示明确状态；
- 失败时不假装连接成功；
- 支持 Option/Command 快捷入口（P1）。

---

# 27. 主界面信息架构

```text
Dashboard
Nodes
Subscriptions
Policies          [V0.5]
Rules
DNS

Modules           [V1.0]
Scripts           [V1.0]
Rewrite           [V1.0]
MITM              [V1.0]

Connections       [V0.5]
Traffic
Logs

Settings
```

未实现的 V1.0 功能在 V0.1 不显示灰色“占位菜单”，避免产品显得未完成。

---

# 28. Dashboard 设计

核心卡片：

```text
Status       Connected
Node         US-SJ-01
Protocol     Hysteria2
Core         sing-box
Latency      32 ms
Mode         Rule
TUN          ON
DNS          Enhanced
Download     18.4 MB/s
Upload       3.2 MB/s
```

快捷操作：

- Connect/Disconnect；
- Switch Node；
- Test Current Node；
- Open Logs；
- Toggle TUN；
- Update Subscriptions。

---

# 29. 网络变化与恢复

使用 `NWPathMonitor` 监听可用网络路径变化；Apple 文档将其定义为用于监测和响应网络变化的 observer [R10]。

需要处理：

```text
Wi-Fi → Personal Hotspot
Wi-Fi → Ethernet
Ethernet → Wi-Fi
Sleep → Wake
IP Change
DNS Change
Default Route Change
```

恢复策略：

```text
Path Changed
   ↓
Debounce 500~1500ms
   ↓
Check Core Health
   ↓
Check Tunnel State
   ↓
Refresh DNS/Route if needed
   ↓
Reconnect only when necessary
```

禁止每次轻微 `NWPath` 变化都重启 Core。

---

# 30. 开机启动与生命周期

设置项：

- Launch at Login；
- Connect on Launch；
- Restore Last Node；
- Restore Last Mode；
- Auto Update Subscription；
- Minimize to MenuBar；
- Quit Behavior：Disconnect / Keep Tunnel（根据系统能力决定）。

App 退出前顺序：

```text
Stop monitoring writes
↓
Stop/hand off tunnel
↓
Stop Core
↓
Restore system proxy if managed
↓
Flush traffic aggregation
↓
Exit
```

---

# 31. Module Engine（V1.0）

## 31.1 定位

Module 是可选扩展层，不影响 V0.1 代理主链路。

内部格式建议优先定义 `Magies Module`，再增加 Shadowrocket/Surge/Loon 兼容解析器。

## 31.2 Module 数据结构

```swift
struct MagiesModule: Identifiable, Codable {
    let id: UUID
    var name: String
    var description: String
    var enabled: Bool
    var source: ModuleSource
    var mitm: MITMConfig?
    var rewriteRules: [RewriteRule]
    var scripts: [ScriptRule]
    var arguments: [String: String]
    var permissions: ModulePermissions
}
```

## 31.3 模块导入

支持：

- 本地文件；
- URL；
- 剪贴板；
- 后续模块订阅。

导入前必须展示：

- 来源；
- MITM hostname；
- 是否修改请求；
- 是否修改响应；
- 是否访问网络；
- 是否读写 Persistent Store。

---

# 32. Script Engine（V1.0）

使用 JavaScriptCore 作为原生 JS 执行环境。JavaScriptCore 提供 `JSContext` 等能力，可在 App 内创建独立 JavaScript 上下文 [R11]。

## 32.1 兼容 API 初版

```javascript
$request
$response
$done()
$argument
$persistentStore
$notification
$httpClient
```

## 32.2 Sandbox

必须限制：

- 执行超时；
- 同时执行数量；
- 内存增长；
- 文件访问；
- 任意进程执行；
- 任意 Keychain 访问；
- 默认网络权限。

脚本不能直接拿到 App 的任意 Swift 对象。

## 32.3 Persistent Store

每个 Module 独立 namespace：

```text
module.<module-id>.<key>
```

防止模块之间读写对方数据。

---

# 33. Rewrite Engine（V1.0）

支持：

- URL Rewrite；
- Redirect；
- Reject；
- Request Header；
- Response Header；
- Request Body；
- Response Body；
- HTTP Status；
- Script Hook。

规则流程：

```text
Request
 ↓
Pre-Request Rewrite
 ↓
Request Script
 ↓
Upstream
 ↓
Response
 ↓
Response Rewrite
 ↓
Response Script
 ↓
Client
```

规则匹配必须有：

- Regex 编译缓存；
- 最大 body 限制；
- timeout；
- binary/text 区分。

---

# 34. MITM（V1.0）

## 34.1 原则

MITM 默认关闭，并且必须由用户主动：

1. 生成 CA；
2. 安装 CA；
3. 信任 CA；
4. 配置 hostname 白名单；
5. 开启模块。

## 34.2 组件

```text
CertificateManager
Root CA
Leaf Certificate Cache
TLS Server Side
TLS Upstream Client
HTTP/1.1 Parser
HTTP/2 Support
Rewrite Pipeline
Script Pipeline
```

## 34.3 安全约束

- 不允许默认 `hostname = *`；
- CA 私钥存 Keychain；
- 导出 CA 私钥需要二次确认；
- MITM 日志默认不保存敏感 Body；
- 对证书固定（certificate pinning）的应用不承诺透明兼容；
- 模块只能拦截其声明的 hostname。

---

# 35. WLOC 等高级模块的定位

如果后续使用 WLOC 作为 Module Engine 验证项目，应当走完整模块链路：

```text
HTTPS Response
    ↓
MITM whitelist matched
    ↓
Binary Body
    ↓
Script Engine
    ↓
protobuf parse / patch / encode
    ↓
Modified Response
```

**不得在 CoreManager 或 TunnelService 中写死 WLOC 逻辑。**

是否能改变特定系统服务行为，应以实际 macOS 抓包、系统链路和目标应用行为验证为准，不作为 V1.0 主产品验收前提。

---

# 36. 安全与隐私设计

## 36.1 Local First

默认不上传：

- 节点；
- 订阅；
- 路由规则；
- DNS 查询；
- Connections；
- 访问域名；
- Module 数据。

## 36.2 最小权限

- 主 App 只申请必要能力；
- Network Extension 独立；
- Keychain Access Group 只在主 App 与 Extension 必须共享时启用；
- MITM 能力与基础代理权限分开。

## 36.3 Threat Model

重点威胁：

1. 恶意订阅注入异常配置；
2. 恶意 Module 执行脚本；
3. Core 二进制被替换；
4. 更新源被劫持；
5. 日志泄露密钥；
6. MITM CA 私钥泄露；
7. 异常退出留下系统代理导致断网。

对应措施：

- 下载与 Core hash 校验；
- 签名/版本固定；
- Script Sandbox；
- 日志脱敏；
- Keychain；
- System Proxy Recovery；
- 更新回滚。

---

# 37. 开源许可证与分发风险

该项目必须在正式分发前单独做一次 License Review。

当前参考：

- v2rayN 仓库标注 GPL-3.0 [R12]；
- sing-box 项目使用 GPL-3.0-or-later 相关许可声明 [R13]；
- Xray-core 仓库标注 MPL-2.0 [R14]。

因此：

1. **不要直接复制 v2rayN GPL 源码进入闭源 App 后假设无影响**；
2. 如果 Magies Proxy 未来闭源或商业化，应明确区分“独立进程调用”“代码链接”“修改并分发 Core”等场景；
3. Core 二进制随 App 分发、自动下载或由用户自行安装，对许可证义务可能不同；
4. 最终方案应由熟悉开源许可的软件法律顾问复核。

本 PRD 只给出工程风险提示，不构成法律意见。

---

# 38. Intel x86_64 策略

## 38.1 构建产物

V0.1：

```text
MagiesProxy-Intel.dmg
Architecture: x86_64
```

后续：

```text
MagiesProxy-AppleSilicon.dmg
MagiesProxy-Universal.dmg
```

## 38.2 Core 验证

CI 每次升级 Core 必须执行：

```text
1. 检查 darwin-amd64 artifact
2. 检查 SHA256
3. --version 可执行
4. 最小配置 validate
5. 本地 SOCKS/HTTP smoke test
6. TUN integration test（可在专用 Runner）
```

v2rayN 官方发行说明当前仍列出 macOS x64 包 [R3]；Xray 官方 release workflow 也包含 `darwin/amd64` [R4]。但项目不能把未来 Intel artifact 永久存在当成无条件前提。

---

# 39. macOS 最低版本

建议产品基线：

> **macOS 13+，Intel x86_64 首发。**

理由：

- 减少过旧系统兼容成本；
- SwiftUI、Concurrency、NetworkExtension 等使用体验更稳定；
- 便于统一测试矩阵。

如果确实需要覆盖 macOS 12，可在 V0.1 技术 Spike 后确认 API 和部署工具链成本再决定。

---

# 40. 配置生成流水线

```text
Database Domain Model
       ↓
RuntimeProfileBuilder
       ↓
Capability Validation
       ↓
Core Config Generator
       ↓
JSON / Runtime Files
       ↓
Core Validate
       ↓
Atomic Replace Runtime Directory
       ↓
Start/Reload Core
```

配置生成必须是纯函数式/可测试的：

```swift
func generate(profile: RuntimeProfile) throws -> CoreConfiguration
```

尽量避免生成过程中直接修改全局状态。

---

# 41. 内部服务接口

## 41.1 ProfileService

```swift
protocol ProfileService {
    func activeProfile() async throws -> RuntimeProfile
    func selectNode(_ id: UUID) async throws
    func setRoutingMode(_ mode: RoutingMode) async throws
}
```

## 41.2 SubscriptionService

```swift
protocol SubscriptionService {
    func refresh(id: UUID) async throws -> SubscriptionUpdateResult
    func refreshAll() async -> [SubscriptionUpdateResult]
}
```

## 41.3 TunnelService

```swift
protocol TunnelService {
    func installConfiguration() async throws
    func start(profileID: UUID) async throws
    func stop() async
    func status() async -> TunnelStatus
}
```

## 41.4 MonitoringService

```swift
protocol MonitoringService {
    func trafficStream() -> AsyncStream<TrafficSnapshot>
    func coreHealthStream() -> AsyncStream<CoreHealth>
}
```

---

# 42. 错误模型

统一错误，不直接向 UI 暴露 Core stderr 原文作为唯一提示。

```swift
enum MagiesError: Error {
    case invalidNode(reason: String)
    case subscriptionDownloadFailed
    case subscriptionParseFailed
    case coreMissing(CoreType)
    case coreArchitectureMismatch
    case coreValidationFailed(details: String)
    case coreStartFailed(details: String)
    case localPortOccupied(Int)
    case systemProxyUpdateFailed
    case tunnelPermissionDenied
    case tunnelStartFailed(details: String)
    case dnsConfigurationInvalid
    case ruleConfigurationInvalid
    case keychainFailure
}
```

UI 显示：

```text
发生了什么
为什么可能发生
用户下一步可以做什么
技术详情（可展开）
```

---

# 43. 设置项

## General

- Launch at Login；
- Connect on Launch；
- Restore Last Node；
- MenuBar Only；
- Language；
- Update Channel。

## Proxy

- Core selection；
- HTTP Port；
- SOCKS Port；
- Routing Mode；
- UDP；
- IPv6。

## TUN

- Enable；
- MTU；
- Auto Route；
- DNS Hijack；
- IPv6。

## DNS

- Profile；
- FakeIP；
- DoH/DoT；
- Bootstrap。

## Logs

- Level；
- Max File Size；
- Retention；
- Export diagnostics。

## Advanced

- Core version；
- Runtime directory；
- Reset System Proxy；
- Reset VPN configuration；
- Reset database（危险操作二次确认）。

---

# 44. 更新机制

四类更新分离：

```text
App Update
Core Update
Rule/Geo Update
Module Update
```

## 44.1 Core 更新

- 读取版本 manifest；
- 下载对应架构；
- SHA256；
- 验证 `--version`；
- 测试配置；
- 原子替换；
- 保留上一版本；
- 失败回滚。

## 44.2 App 更新

正式分发阶段根据签名方式选择 Sparkle、App Store 或自有更新方案。

---

# 45. 测试策略

## 45.1 Unit Tests

覆盖：

- URI Parser；
- Subscription Parser；
- Domain Model；
- Config Generator；
- Rule ordering；
- DNS builder；
- Secret masking；
- Module Parser（V1.0）。

目标核心模块行覆盖率 >= 80%，但不以覆盖率代替场景测试。

## 45.2 Integration Tests

- 启动 Xray；
- 启动 sing-box；
- 本地 HTTP/SOCKS；
- 节点切换；
- 配置 reload；
- Core crash recovery；
- Subscription transaction；
- System Proxy save/restore。

## 45.3 TUN Tests

专门测试：

- start/stop 100 次；
- Wi-Fi 切热点；
- 睡眠 30 分钟后唤醒；
- DNS 切换；
- IPv6 网络；
- 网络断开再恢复；
- Core crash 时 Tunnel 行为。

## 45.4 UI Tests

关键主流程：

```text
首次启动 → 导入订阅 → 选节点 → 连接 → 切节点 → 断开
```

---

# 46. 性能测试

## 46.1 基准

需要与“直接运行 Core”做对照，Magies Proxy 自身不应明显降低吞吐。

测试：

- 1 Gbps 局域网环境；
- 10k 并发短连接（能力范围内）；
- 大文件持续下载；
- UDP/QUIC；
- Hysteria2；
- TUN vs System Proxy。

## 46.2 UI 性能

Connections 高并发时 UI 使用批量刷新：

- 250~500ms 合并更新；
- 不为每条流量事件触发 SwiftUI 全树刷新。

---

# 47. CI/CD

推荐流水线：

```text
Pull Request
 ↓
SwiftLint / Format
 ↓
Unit Tests
 ↓
Build x86_64
 ↓
Core Artifact Check
 ↓
Config Generator Golden Tests
 ↓
Integration Smoke Tests
 ↓
Signed/Nightly Artifact
```

Release：

```text
Tag
 ↓
Build Intel
 ↓
Build arm64（后续）
 ↓
Universal（后续）
 ↓
Codesign
 ↓
Notarize
 ↓
DMG
 ↓
SHA256 + Release Notes
```

---

# 48. 签名、权限与分发

需要尽早验证：

- Apple Developer ID；
- Network Extension entitlement；
- App Group；
- Keychain Sharing（必要时）；
- Hardened Runtime；
- Codesign；
- Notarization；
- DMG 安装。

Apple 对 Network Extension provider 的部署方式有专门说明 [R7]，因此不能把 Debug 环境能跑等同于正式签名分发一定能跑。

---

# 49. 开发阶段与里程碑

## Phase 0 - 技术可行性 Spike

必须完成：

- Intel Xcode build；
- 最小 SwiftUI App；
- `NEPacketTunnelProvider` 启停；
- Xray darwin-amd64 启动；
- sing-box darwin-amd64 启动；
- 主 App 与 Extension IPC；
- 基础签名验证。

**退出条件：** 所有 P0 基础技术无阻断项。

## Phase 1 - Core + Node

- CoreManager；
- ProxyNode；
- VLESS/VMess/Trojan/SS/HY2 Parser；
- 手工节点；
- HTTP/SOCKS 代理；
- 日志。

**退出条件：** 不使用 TUN 时可稳定代理。

## Phase 2 - Subscription + System Proxy

- Subscription；
- 去重；
- 更新事务；
- System Proxy；
- 菜单栏。

**退出条件：** 具备 v2rayN 基础日常使用主流程。

## Phase 3 - TUN + DNS + Route

- NetworkExtension；
- TUN；
- DNS；
- Global/Rule/Direct；
- Geo 规则。

**退出条件：** 可以作为主力全局代理客户端。

## Phase 4 - V0.1 稳定化

- Sleep/Wake；
- NWPath 恢复；
- Crash Recovery；
- 流量；
- 测速；
- Intel DMG；
- 72 小时测试。

**退出条件：** V0.1 正式验收。

## Phase 5 - V0.5

- Policy Group；
- Connections；
- Rule Provider；
- Process rule；
- 自动故障切换。

## Phase 6 - V1.0 Module

- Module Parser；
- Rewrite；
- JavaScriptCore；
- MITM；
- Binary Body；
- 高级模块验证。

---

# 50. V0.1 验收清单

以下项目必须全部通过：

- [ ] Intel x86_64 安装成功；
- [ ] App 正常签名/运行；
- [ ] Xray 启动/停止；
- [ ] sing-box 启动/停止；
- [ ] VLESS；
- [ ] VMess；
- [ ] Trojan；
- [ ] Shadowsocks；
- [ ] Hysteria2；
- [ ] 分享链接导入；
- [ ] 订阅导入与更新；
- [ ] 节点增删改查；
- [ ] 延迟测试；
- [ ] URL Test；
- [ ] HTTP/SOCKS System Proxy；
- [ ] 原系统代理保存和恢复；
- [ ] TUN；
- [ ] IPv4；
- [ ] DNS；
- [ ] Global；
- [ ] Rule；
- [ ] Direct；
- [ ] 菜单栏；
- [ ] 日志；
- [ ] 实时速率；
- [ ] 睡眠唤醒恢复；
- [ ] Wi-Fi/热点切换恢复；
- [ ] Core crash 有限重试；
- [ ] 72 小时稳定性测试。

---

# 51. Definition of Done

任何 P0 功能只有同时满足以下条件才算完成：

1. 正常路径可用；
2. 失败路径有明确错误；
3. 有 Unit/Integration Test；
4. Intel 设备验证；
5. 不泄露 secret 到日志；
6. 不破坏 System Proxy/TUN 的恢复能力；
7. UI 状态与真实 Core/Tunnel 状态一致；
8. 文档或代码注释记录关键约束。

---

# 52. 风险登记

| 风险 | 概率 | 影响 | 处理 |
|---|---|---|---|
| NetworkExtension entitlement/分发复杂 | 中 | 高 | Phase 0 提前验证 |
| Intel Core 未来停止发布 | 中 | 高 | 版本固定、CI 检查、保留已验证 Core |
| TUN 网络恢复不稳定 | 中 | 高 | 独立状态机 + 长时间测试 |
| DNS 回环/泄漏 | 中 | 高 | 配置验证 + DNS 测试矩阵 |
| 多 Core 配置差异膨胀 | 高 | 中 | Unified Model + Adapter |
| MITM 开发量远超预期 | 高 | 中 | 放到 V1.0，不阻塞 V0.1 |
| Script 安全问题 | 中 | 高 | Sandbox + 权限模型 |
| 开源许可证影响商业分发 | 中 | 高 | 分发前 License Review |
| UI Connections 高频刷新卡顿 | 中 | 中 | 聚合刷新 + diff |
| System Proxy 异常残留 | 中 | 高 | 原配置快照 + 启动恢复 |

---

# 53. 需要尽早验证的技术决策

## 决策 A：TUN 中 Core 的运行位置

选项：

- Core 运行在主 App 辅助进程，PacketTunnel 只转发；
- Core 能力直接集成/运行在 Extension 允许的架构内；
- 采用 sing-box Apple 相关实现方式进行桥接。

**要求：** Phase 0 做 PoC 后锁定，不能凭想象决定。

## 决策 B：Core 随包还是首次下载

V0.1 推荐：

- 内置“已验证版本”，降低首次使用失败率；
- 后续提供 Core 独立更新。

但最终方式必须结合许可证与 App 包体积确定。

## 决策 C：最低 macOS

默认 macOS 13+；如必须兼容 12，再用真实 Intel 设备测试后决定。

## 决策 D：Module 格式

推荐先定义 Magies 内部 AST，再做外部格式适配：

```text
Shadowrocket Parser ─┐
Surge Parser ────────┼─> Magies Module AST ─> Engine
Loon Parser ─────────┤
Magies Parser ───────┘
```

---

# 54. 推荐的首批开发任务拆解

## Epic A - Project Foundation

- A01 建 Xcode Workspace；
- A02 Swift Package 分层；
- A03 日志组件；
- A04 SQLite 基础层；
- A05 Keychain Wrapper；
- A06 App 状态管理。

## Epic B - Core Runtime

- B01 Core binary locator；
- B02 架构检测；
- B03 Xray adapter；
- B04 sing-box adapter；
- B05 Process runner；
- B06 stdout/stderr stream；
- B07 health check；
- B08 crash recovery。

## Epic C - Node & Subscription

- C01 Unified node model；
- C02 VLESS Parser；
- C03 VMess Parser；
- C04 Trojan Parser；
- C05 SS Parser；
- C06 HY2 Parser；
- C07 Subscription fetch；
- C08 Subscription transaction；
- C09 node dedup。

## Epic D - Proxy Runtime

- D01 local SOCKS；
- D02 local HTTP；
- D03 System Proxy read/write；
- D04 System Proxy recovery；
- D05 port conflict check。

## Epic E - Tunnel

- E01 PacketTunnel Target；
- E02 entitlement；
- E03 VPN config install；
- E04 app-extension IPC；
- E05 route settings；
- E06 DNS settings；
- E07 start/stop state machine。

## Epic F - UI

- F01 Dashboard；
- F02 Nodes；
- F03 Subscriptions；
- F04 Rules；
- F05 DNS；
- F06 Logs；
- F07 Settings；
- F08 MenuBar。

## Epic G - Stability

- G01 NWPathMonitor；
- G02 sleep/wake；
- G03 auto reconnect；
- G04 traffic aggregation；
- G05 diagnostic export；
- G06 72h soak test。

---

# 55. 推荐开发顺序

严格按以下顺序：

```text
1. Intel + NetworkExtension Spike
2. CoreRunner
3. 单节点代理
4. Unified Node Model
5. 订阅
6. System Proxy
7. TUN
8. DNS
9. Route
10. MenuBar
11. Traffic / Health / Recovery
12. V0.1 Release
13. Policies / Connections
14. Module / Script / MITM
```

不要先做漂亮 Dashboard，再解决 NetworkExtension。

不要先做 WLOC，再解决完整代理主链路。

---

# 56. 最终产品演进

## V0.1 - “可替代 v2rayN”

核心关键词：

```text
Intel
Xray
sing-box
Node
Subscription
System Proxy
TUN
DNS
Rules
Logs
MenuBar
```

## V0.5 - “更像高级 macOS 网络工具”

```text
Policies
URL-Test
Fallback
Load Balance
Connections
Process Rules
Rule Provider
```

## V1.0 - “具备 Shadowrocket 类扩展能力”

```text
Module
Rewrite
Script
JavaScriptCore
MITM
Binary Body
```

最终产品定义：

> **Magies Proxy = macOS 原生体验 + v2rayN 级核心代理能力 + Shadowrocket 风格扩展能力。**

---

# 57. 参考资料

> 以下资料用于确认技术路线与平台能力，核实日期 2026-08-08。

**[R1] Apple Developer Documentation - NEPacketTunnelProvider**  
https://developer.apple.com/documentation/networkextension/nepackettunnelprovider

**[R2] Apple Developer Documentation - Packet tunnel provider**  
https://developer.apple.com/documentation/networkextension/packet-tunnel-provider

**[R3] v2rayN Wiki - Release files introduction**  
https://github.com/2dust/v2rayN/wiki/Release-files-introduction

**[R4] XTLS/Xray-core - release.yml**  
https://github.com/XTLS/Xray-core/blob/main/.github/workflows/release.yml

**[R5] sing-box - Apple platforms**  
https://sing-box.sagernet.org/clients/apple/

**[R6] sing-box - Headless Rule / process rules**  
https://sing-box.sagernet.org/configuration/rule-set/headless-rule/

**[R7] Apple Technote TN3134 - Network Extension provider deployment**  
https://developer.apple.com/documentation/technotes/tn3134-network-extension-provider-deployment

**[R8] Apple Developer Documentation - Keychain Services**  
https://developer.apple.com/documentation/security/keychain-services

**[R9] Apple Security - Keychain**  
https://developer.apple.com/security/

**[R10] Apple Developer Documentation - NWPathMonitor**  
https://developer.apple.com/documentation/network/nwpathmonitor

**[R11] Apple Developer Documentation - JavaScriptCore / JSContext**  
https://developer.apple.com/documentation/javascriptcore/jscontext

**[R12] 2dust/v2rayN License**  
https://github.com/2dust/v2rayN/blob/master/LICENSE

**[R13] SagerNet/sing-box**  
https://github.com/SagerNet/sing-box

**[R14] XTLS/Xray-core License**  
https://github.com/XTLS/Xray-core/blob/main/LICENSE

---

# 58. 结语

Magies Proxy 第一版不追求功能数量，而追求**主链路完整性**：

```text
导入节点/订阅
      ↓
生成配置
      ↓
启动 Core
      ↓
System Proxy / TUN
      ↓
DNS + Routing
      ↓
稳定运行
      ↓
监控 + 恢复
```

只要 V0.1 做稳，后面的 Policy、Connections、Module、Rewrite、MITM 都可以作为独立层逐步增加。

因此项目长期技术原则保持不变：

> **Proxy First, Module Later.**  
> **Intel First, Universal Later.**  
> **Unified Model First, Multi-Core Behind It.**  
> **Local First, Security by Default.**
