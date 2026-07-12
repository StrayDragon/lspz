# Design: fix-mcp-accept-empty-diagnostics

Align `McpServer` diagnostics wait with `DaemonMcpServer`:

```text
on publishDiagnostics for target uri:
  return immediately  // empty or non-empty both terminal
```

Remove the “Empty diagnostics — keep waiting within budget” loop branch.
