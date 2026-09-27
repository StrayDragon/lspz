# language: zh-CN
# capability: mcp
# purpose: MCP 服务器集成，将 LSP 能力暴露为 MCP 工具供 Claude Desktop 等客户端使用。
# scope: src/mcp/, tests/

功能: mcp

  @req:r19
  规则: MCP 服务器
    系统 MUST 实现 MCP 服务器协议，暴露 LSP 能力为 MCP 工具。
  @req:r29
  规则: 工具注册
    系统 MUST 注册 get_diagnostics、get_completions、get_symbols 等 MCP 工具。
  @req:r38
  规则: 会话管理
    系统 MUST 通过 LspPool 管理多个 LSP 会话（按语言/工作区复用）。
  @req:r47
  规则: 自动语言检测
    系统 MUST 根据文件扩展名自动检测语言和对应的 LSP 后端。
  @req:r54
  规则: Daemon 优雅退出
    Daemon MUST 在 SIGINT、idle 超时与 daemon/shutdown 时先清空 LSP 会话池（触发子进程 Drop/kill）并删除真实 socket 文件，禁止在未回收子进程时直接 process::exit。
  @req:r61
  规则: Shutdown 使用真实 socket 路径
    daemon/shutdown MUST 使用服务器持有的 socket_path 删除套接字文件，不得依赖可能未设置的 LSPZ_SOCKET 环境变量。
  @req:r68
  规则: 文档同步 SSOT
    系统 MUST 通过 LspSession::open_or_update_document 作为文档打开/更新的唯一实现；MCP daemon 路径 MUST 经 daemon RPC 调用该逻辑，禁止固定 didOpen version=1。
  @req:r71
  规则: Per-session 锁
    系统 MUST 在 LSP 请求/通知等待期间仅锁定目标会话；get_or_spawn 的 initialize MUST NOT 在持有全局池锁时 await，以免阻塞其他会话查找。
  @req:r73
  规则: 请求等待期间缓冲通知
    LspSession::send_request MUST 将交错的服务器通知缓冲；wait_for_notification 系列 MUST 优先消费缓冲，不得静默丢弃。
  @req:r10
  规则: Daemon 状态计数
    DaemonStatus MUST 在接受连接时递增 total_connections、在分发请求时递增 total_requests，并在 daemon/status 中报告自启动以来的 uptime_secs。
  @req:r11
  规则: 接受首个空诊断通知
    In-process MCP get_diagnostics MUST 与 daemon 路径一致：将目标 URI 的首个 publishDiagnostics（可为空数组）视为终态，禁止为等待非空诊断而空转直至超时。
  @req:r12
  规则: file URI 百分号解码
    系统 MUST 在将 file:// URI 转为本地路径前进行百分号解码，使含空格/非 ASCII 的路径可被读取。
  @req:r13
  规则: 回收已死 LSP 子进程
    LspPool 在查找/复用会话前 MUST 检测子进程已退出（try_wait），并移除死会话以便重新 spawn，不得一直复用到 idle reaper。
  @req:r14
  规则: owns_daemon Drop 关闭
    DaemonClient 在 owns_daemon=true 被 Drop 时 MUST 尽力发送 daemon/shutdown（或等价终止），不得仅依赖 10min idle TTL。
  @req:r75
  规则: MCP Roots 与 peer 上下文
    MCP 服务器在客户端完成连接/initialize 后 MUST 获取并缓存 MCP Roots（若客户端声明该能力），并保留 peer_info/client_info；Roots 请求失败时 MUST WARN 并 fail-open（不中断会话），不得假装已绑定工作区。
  @req:r76
  规则: 可选 URI 与工作区解析
    MCP 工具 get_diagnostics 与 get_symbols MUST 支持可选 uri、path、paths 与 workspace；相对 path/paths MUST 相对已解析的 workspace 锚点解析为 file://。锚点优先级 MUST 为：工具 workspace 参数 → 会话 set_workspace 绑定 → MCP Roots → 可信进程/daemon cwd（含项目 marker 且非 home/文件系统根）。当 uri/path/paths 皆缺省时，仅在锚点可信时允许工作区扫描。
  @req:r77
  规则: 无 URI 诊断摘要流
    当 get_diagnostics 在无有效 uri 且已解析到工作区时，系统 MUST 扫描/触发该工作区内相关 LSP 诊断工作，并返回面向 Agent 的 max-column/compact TOON 摘要（含截断上限），禁止因摘要失败而中断 MCP 会话（失败时 MUST 返回可行动错误文本）。
  @req:r78
  规则: Cursor 类客户端兼容
    系统 MUST 支持 Cursor 等编码 Agent MCP 客户端：在其提供 Roots（及可识别的 client/peer 信息）时，允许省略 file:// uri 完成默认诊断摘要流；不得要求此类客户端为默认诊断调用填写绝对 URI。
  @req:r79
  规则: Roots 内路径约束
    当会话已缓存非空 MCP Roots 时，显式 file://（或规范化后的本地路径）若落在所有 root 之外，工具调用 MUST 拒绝并返回明确错误；不得静默读取 Roots 外路径。无 Roots 时行为保持现状（不在本需求扩大为全局沙箱）。
  @req:r85
  规则: 相对路径与多文件目标
    当工具收到 path 或 paths（相对或绝对）时，系统 MUST 将其解析为本地文件 URI 并执行对应诊断/符号查询；多目标时 MUST 返回与 workspace scan 同形的 TOON 概览。相对路径在无 workspace 锚点时 MUST 返回明确错误，不得猜测 $HOME。
  @req:r86
  规则: 会话 set_workspace
    系统 MUST 提供 set_workspace 工具，接受绝对路径或 file:// URI，将会话绑定到该工作区根；后续工具调用在未显式传 workspace 时 MUST 优先使用该绑定解析相对路径与默认扫描。
  @req:r87
  规则: 不可信 fallback 拒绝扫描
    当 workspace 锚点来源为 fallback 或 process_cwd，且路径为用户 home、文件系统根、或缺少项目 marker（含 .git 与既有 ROOT_MARKERS）时，无目标文件的扫描 MUST 拒绝并返回可行动错误（提示传 workspace、set_workspace、绝对 uri/path，或确保客户端 Roots），禁止扫描该目录。
  @req:r88
  规则: 未知工具参数拒绝
    MCP 工具入参反序列化 MUST 拒绝未知字段（deny_unknown_fields），返回明确错误；不得静默忽略错误字段名（例如 paths 拼写正确但在旧版本曾被忽略的场景由本需求保证可见失败）。
  @req:r89
  规则: Roots best-effort 与诊断日志
    系统 MUST 继续在客户端声明 roots 能力时尝试 roots/list 作为锚点；失败或未声明时 MUST 记录含 client 名与原因的 INFO/WARN 日志，不得伪称已绑定 MCP roots（workspace_source 不得为 mcp_roots）。
  @req:r19
  规则: mcp-start
    MUST hold: Given MCP 服务器配置; When 启动 lspz mcp; Then 服务器开始监听 MCP 请求.
  @req:r29
  规则: tool-diagnostics
    MUST hold: Given MCP 客户端调用 get_diagnostics; When MCP 服务器处理; Then 返回压缩后的诊断信息.
  @req:r29
  规则: tool-completions
    MUST hold: Given MCP 客户端调用 get_completions; When MCP 服务器处理; Then 返回压缩后的补全信息.
  @req:r38
  规则: session-reuse
    MUST hold: Given 相同工作空间的多个请求; When LspPool 处理; Then 复用现有 LSP 会话.
  @req:r47
  规则: auto-detect-rs
    MUST hold: Given .rs 文件; When 工具调用; Then 自动使用 rust-analyzer.
  @req:r54
  规则: graceful-sigint
    MUST hold: Given Daemon 持有已 spawn 的 LSP 子进程; When 收到 SIGINT 或 daemon/shutdown; Then 会话池被清空、socket 文件被删除、子进程被终止后再退出.
  @req:r61
  规则: socket-path
    MUST hold: Given Daemon 以 --socket 路径启动且未设置 LSPZ_SOCKET; When 客户端调用 daemon/shutdown; Then 该 socket 文件被删除.
  @req:r68
  规则: daemon-sync
    MUST hold: Given 同一 URI 第二次工具调用; When DaemonMcpServer 同步文档; Then 会话发送 didChange 且 version 递增而非再次 didOpen.
  @req:r71
  规则: spawn-no-global-await
    MUST hold: Given 会话 A 正在 initialize; When 会话 B 查找已存在的 key; Then B 不被 A 的 initialize await 阻塞.
  @req:r73
  规则: buffer-during-request
    MUST hold: Given send_request 等待响应期间收到 publishDiagnostics; When 随后 wait_for_notification publishDiagnostics; Then 返回先前缓冲的通知 params.
  @req:r10
  规则: status-counters
    MUST hold: Given Daemon 已运行并处理过连接与请求; When daemon/status; Then total_connections 与 total_requests 大于 0 且 uptime_secs 大于等于 0.
  @req:r11
  规则: empty-is-terminal
    MUST hold: Given 干净源文件首次 publishDiagnostics 为空; When get_diagnostics; Then 立即返回空结果且不消耗完整 10s 预算.
  @req:r12
  规则: decode-space
    MUST hold: Given URI file:///tmp/my%20file.rs; When MCP 读取文件内容; Then 成功打开 /tmp/my file.rs.
  @req:r13
  规则: reap-dead
    MUST hold: Given 池中会话的 LSP 子进程已退出; When 再次 get_or_spawn 同 key; Then 旧会话被移除并创建新会话.
  @req:r14
  规则: drop-shutdown
    MUST hold: Given MCP 客户端 auto-start 了 daemon; When 客户端 Drop; Then 向 daemon 发出 shutdown 请求.
  @req:r75
  规则: roots-cached
    MUST hold: Given Cursor 类客户端已连接且声明 Roots 能力; When MCP initialize/连接完成; Then 服务器缓存至少一条 root URI 并记录 client/peer 标识.
  @req:r75
  规则: roots-fail-open
    MUST hold: Given 客户端声明 Roots 但 list/roots 请求失败; When 连接完成; Then 会话仍可用且日志含 WARN，不伪称已绑定 root.
  @req:r76
  规则: relative-path-with-workspace
    MUST hold: Given 锚点为 /proj; When get_diagnostics path=src/main.rs; Then 打开 /proj/src/main.rs 并返回诊断 TOON.
  @req:r76
  规则: explicit-workspace-param
    MUST hold: Given 无会话绑定; When get_diagnostics workspace=/proj path=a.ts; Then 使用 /proj 解析 a.ts.
  @req:r77
  规则: workspace-toon-summary
    MUST hold: Given 工作区已解析且 LSP 可返回诊断; When 无 uri 调用 get_diagnostics; Then 返回 compact/max-column TOON 摘要且行数不超过配置上限.
  @req:r78
  规则: cursor-omit-uri
    MUST hold: Given peer/client 标识为 Cursor 类且 Roots 非空; When 无参或空 uri 调用 get_diagnostics; Then 成功返回摘要而非 uri required 校验错误.
  @req:r79
  规则: outside-roots-reject
    MUST hold: Given 已缓存 root file:///proj; When 调用工具 uri=file:///etc/passwd; Then 返回明确越界错误且不读取该文件.
  @req:r85
  规则: multi-paths-overview
    MUST hold: Given workspace=/proj 且 paths 含两文件; When get_diagnostics; Then 返回 scanned 概览 TOON 含两行 path.
  @req:r85
  规则: relative-without-anchor
    MUST hold: Given 无 roots/会话/显式 workspace; When get_diagnostics path=src/a.rs; Then 返回明确错误要求 workspace 或绝对路径.
  @req:r86
  规则: set-then-relative
    MUST hold: Given 已 set_workspace=/proj; When get_diagnostics path=src/main.rs; Then 成功解析到 /proj/src/main.rs.
  @req:r87
  规则: home-fallback-reject
    MUST hold: Given 无 roots 且 cwd 为 $HOME; When 无 uri/path 调用 get_diagnostics; Then 返回错误且不扫描 home 下文件.
  @req:r88
  规则: unknown-field-reject
    MUST hold: Given 参数含 typo_field; When 调用 get_diagnostics; Then 反序列化失败并返回 invalid_request.
  @req:r89
  规则: roots-missing-logged
    MUST hold: Given 客户端未声明 roots; When resolve_workspace; Then 日志含未声明能力原因且 source 非 mcp_roots.
