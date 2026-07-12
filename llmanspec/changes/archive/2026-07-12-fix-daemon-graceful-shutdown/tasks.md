# Tasks: fix-daemon-graceful-shutdown

- [x] 1. Add LspPool::clear() to drop all sessions
- [x] 2. Introduce shared shutdown watch/Notify in DaemonServer
- [x] 3. Replace process::exit in ctrl-c, idle reaper, daemon/shutdown with signal + graceful teardown
- [x] 4. daemon/shutdown uses server socket_path
- [x] 5. Unit test: clear empties pool; shutdown path signals watch
- [x] 6. Run just qa + validate
