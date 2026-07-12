# Design: fix-pool-reap-dead-sessions

On lookup: try_lock session → try_wait → Some(status) ⇒ remove from map.
