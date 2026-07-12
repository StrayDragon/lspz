# Design: fix-daemon-client-owned-shutdown

Drop(owns_daemon): spawn thread → current_thread runtime → connect_explicit → shutdown().
