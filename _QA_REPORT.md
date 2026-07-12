`src/` 审查结论：**本地信任模型下 proxy 路径较扎实；daemon/MCP 入口是主要风险面**——未认证的任意进程 spawn、socket TOCTOU、以及压缩逻辑双轨是最该优先处理的。

---

## 1–3. 安全 / 漏洞（合并）

| 严重度 | 位置 | 问题 |
|--------|------|------|
| Critical | `daemon/server.rs` ~53–120, spawn ~299；`mcp/server.rs` / `daemon_server.rs` | Daemon 无认证；`lsp/spawn` + MCP `backend`/`backend_args` 可执行任意命令（argv 执行，非 shell，但仍是 RCE） |
| High | `daemon/server.rs` bind；`daemon/socket.rs` | Socket 无 `chmod 0o600` / `SO_PEERCRED`；可能落到 `/tmp` |
| High | `uri.rs`；MCP/Agent 读文件 | `file://` 无 workspace 沙箱，可读任意本机可读路径 |
| High | `daemon/server.rs` | 任意连接可 `daemon/shutdown`、驱动 LSP session |
| Medium | daemon `read_line`；`timeout_ms` | 无帧大小上限；超大 timeout 可长时间占住 session `Mutex` |
| Medium | TCP/`ws://` | 明文、无认证（outbound client，部署相关） |

**做得好的：** LSP framing 有 `MAX_BODY_SIZE`（16MiB）；spawn 用 `shell_words` + argv（非 `/bin/sh -c`）；proxy fail-open。

---

## 4. 竞态条件

| 严重度 | 位置 | 问题 |
|--------|------|------|
| Critical | `daemon/client.rs` ~50–69；`server.rs` ~53–66 | `exists → remove_file → bind` TOCTOU：并行 auto-start 可留下孤儿 daemon |
| High | `daemon/client.rs` `owns_daemon` Drop | 启动者退出会 `daemon/shutdown`，其它仍在用的 client 被一起干掉 |
| High | `mcp/pool.rs` `get_or_spawn` | 锁外 spawn+initialize：竞态时重复拉起 LSP 子进程，失败者再被 drop/kill |
| Medium | pool idle reaper | 已 clone 的 `Arc` 仍在用时，同 key 可能再插新 session |
| Medium | `config_watcher.rs` | `try_send` 丢事件；无 debounce，编辑器写盘时可能短暂读到半文件 |

---

## 5. 测试不稳定性

| 可能性 | 位置 | 问题 |
|--------|------|------|
| High | `transport/tcp.rs:73` | 固定端口 `127.0.0.1:19876` + `sleep(100ms)`，并行测试易撞端口 |
| Medium | `daemon/server.rs` wait_notify 测试 | 墙钟断言（如 `>=150ms` / `<2s`），CI 负载下易抖 |
| Medium | `mcp/session.rs` empty diagnostics | `elapsed < 500ms` 时序假设 |
| Low | 多数 session/proxy | 已有 30s `timeout`；`init` 用 `ENV_MUTEX` —— 做得较好 |

**缺口：** 无并发 `connect_or_start` / 双 daemon bind / `owns_daemon` Drop 与多 client 共存的测试。

---

## 6. 代码质量与可维护性

**Proxy + interceptor 路径整体清晰**；债务集中在「多入口行为不一致」：

1. **诊断压缩双轨**：`DiagnosticsCompressor`（normalize+dedup）vs MCP 直接 `compact::compress`（无 normalize/dedup）→ SSOT 破坏  
2. **MCP/daemon 绕过 interceptor chain** → 配置开关、capping、metrics 不生效  
3. **TOON 硬编码在 `proxy.rs`** → 新 compressor 必须改 proxy  
4. **`LspSession` 放在 `mcp/`**，却被 agent_sdk / daemon 依赖 → 模块边界错位  
5. **`McpServer` ↔ `DaemonMcpServer` 大量重复**；错误类型 `LspzError` / `anyhow` / `String` 混用  

架构不变量粗评：proxy 路径大体合规；MCP/daemon 路径在 interceptor / config-driven 上明显偏离。

---

## 建议修复优先级

1. Daemon 认证 + socket `0o600` + backend allowlist（堵住 RCE）  
2. Daemon 单飞启动（file lock / 禁止 unlink 活 socket）+ 修正 `owns_daemon` 生命周期  
3. `get_or_spawn` 按 key 单飞；MCP `file://` 限制在 workspace  
4. 统一压缩 SSOT，MCP 走同一套 compressor  
5. TCP 测试改 ephemeral port；去掉脆弱墙钟断言  

更细的分项可看：[安全审查](20c6313d-c772-42e8-89a6-509b31935cd6)、[竞态/测试](0f030921-680f-4df9-9872-48e38a7667bf)、[可维护性](dbac8638-d5d8-4ab9-b347-3b7bd455eff7)。

---

接下来希望我做哪几项？（可多选）

1. 先出 **daemon 认证 + backend allowlist** 的具体改法/补丁  
2. 先修 **socket TOCTOU + `owns_daemon` Drop**  
3. 先修 **`get_or_spawn` 单飞** + 相关测试  
4. 统一 **诊断压缩 SSOT**（让 MCP 走 interceptor）  
5. 修 **不稳定测试**（ephemeral port / 墙钟断言）  
6. 对某一条 finding 做更深的 exploit/复现说明  
7. 以上都先不改，只要再补充某一维度的细节
