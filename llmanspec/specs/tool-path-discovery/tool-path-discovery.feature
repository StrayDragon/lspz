# language: zh-CN
# capability: tool-path-discovery
# purpose: LSP 后端可执行文件路径发现：按 PATH 与包管理器默认布局解析，无需用户 export PATH。
# scope: src/tool_path.rs, src/languages.rs, src/transport/stdio.rs, src/mcp/

功能: tool-path-discovery

  @req:r80
  规则: 后端路径有序发现
    系统 MUST 通过共享 resolve_tool(name) 按固定顺序解析 LSP 后端可执行文件：进程 PATH → $HOME/.local/bin/<name> → 包管理器默认布局（$XDG_DATA_HOME/uv/tools/<pkg>/bin、$CARGO_HOME/bin 或 ~/.cargo/bin、$GOPATH/bin 或 ~/go/bin；可选 npm/bun 全局 bin），不得要求用户为默认安装位置 export PATH。
    场景: path-wins
      假如 PATH 与 ~/.local/bin 均有同名可执行文件
      当 调用 resolve_tool(name)
      那么 返回 PATH 上的路径
    场景: local-bin
      假如 精简 PATH 不含 ~/.local/bin 且工具仅在 ~/.local/bin/<name>
      当 调用 resolve_tool(name)
      那么 返回 ~/.local/bin/<name>
    场景: uv-tools-layout
      假如 精简 PATH 且工具仅在 uv tools 默认 bin 目录
      当 调用 resolve_tool(name)
      那么 返回该 uv tools bin 下的可执行路径
    场景: cargo-bin
      假如 精简 PATH 且工具仅在 ~/.cargo/bin
      当 调用 resolve_tool(name)
      那么 返回 ~/.cargo/bin/<name>
    场景: not-found
      假如 PATH 与所有默认布局均无该工具
      当 调用 resolve_tool(name)
      那么 返回 None 且不 panic

  @req:r81
  规则: languages 与 MCP 使用解析器
    languages 可用性检查与 MCP/daemon 后端 spawn 解析 MUST 使用共享路径解析器，禁止仅依赖精简 PATH 下的 which；当后端仅存在于默认安装布局时仍 MUST 能解析到可执行路径。
    场景: lean-path-detect
      假如 env -i HOME=$HOME PATH=/usr/bin:/bin 且 basedpyright-langserver 仅在 ~/.local/bin
      当 languages 可用性检查或 MCP 解析该后端
      那么 报告已安装或解析到可执行路径

  @req:r82
  规则: 安装文档无需改 PATH
    用户面向安装文档（README 安装节）MUST 说明推荐用 uv tool install（或等价工具安装方式）安装语言服务器，并明确默认布局下无需 export PATH 即可被 lspz 发现。
    场景: readme-uv-tool
      假如 用户阅读 README 安装相关说明
      当 查找语言服务器安装建议
      那么 文档推荐 uv tool install 并说明无需 export PATH
