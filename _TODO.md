
  1. 一键验证

  - just compress-demo: cargo run --example compress-demo -p lspz-core
  - 演示所有 7 个压缩器，显示 token/byte 节省和耗时

  验证结果样本（v0.11+，含全部 7 个压缩器）：
  📦 DiagnosticsCompressor        599B → 242B   (59.6%  81μs)
  📦 CompletionCompressor         561B → 482B   (14.1%  36μs)
  📦 HoverCompressor              214B → 153B   (28.5%  15μs)
  📦 DocumentSymbolCompressor     389B → 163B   (58.1%  13μs)
  📦 LocationCompressor           321B → 234B   (27.1%  19μs)
  📦 WorkspaceSymbolCompressor    342B → 261B   (23.7%  17μs)
  📦 WorkspaceDiagnosticCompressor 475B → 271B   (42.9%  32μs)

  真实场景中 Diagnostics 的 30 条同类告警能到 80% 节省。

  2. 使用建议

  体验压缩效果：
  just compress-demo

  实际使用（代理模式接入 Claude Code 等）：
  lspz --backend rust-analyzer
  lspz --backend gopls            # Go 项目
  lspz --backend basedpyright     # Python 项目

  所有 7 种压缩默认全部开启。可以用 --compress-xxx false 关闭不需要的。
