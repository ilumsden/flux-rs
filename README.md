# Flux-Core Rust API

This repo provides a higher-level, more Rust-native API for Flux-Core.

## Outstanding Flux APIs
The following APIs still need to be implemented. They are listed in order of highest priority to lowest priority.

- Request (`flux_request` family of functions)
- Response (`flux_response` family of functions)
  - Request-to-Response conversion (`flux_response_derive`)
- Message Handler (`flux_msg_handler_t` and associated functions)
- Jobspec and Job (`job.h`, `jobspec1.h`)
- Reactor (`flux_reactor_t`)
  - Integration with async runtimes (e.g., Tokio)
- Modules (i.e., broker module support)
- Jobtap Plugins (`flux_plugin` family of functions)
- Handle (`flux_t`)
  - Flux Attr Cache (`flux_attr_cache_first`, `flux_attr_cache_next`)
  - Host-by-Rank (`flux_get_hostbyrank`)
  - Rank-by-Host (`flux_get_rankbyhost`)
  - Instance Start Time (`flux_get_instance_starttime`)
  - Log redirect (`flux_log_set_redirect`)
  - Message counters (`flux_get_msgcounters`, `flux_clr_msgcounters`)
  - Stats (`flux_stats` family of functions)
- Future (`flux_future_t`)
  - Creation (`flux_future_create`)
  - Aux set/get (`flux_future_aux_get`, `flux_future_aux_set`)
- Message (`flux_msg_t`)
  - Macros for `FLUX_MATCH_ANY`, `FLUX_MATCH_EVENT`, `FLUX_MATCH_REQUEST`, `FLUX_MATCH_RESPONSE`
  - Aux set/get (`flux_msg_aux_get`, `flux_msg_aux_set`)
  - Message get_type/set_type (`flux_msg_set_type`, `flux_msg_get_type`)
  - Get last error (`flux_msg_last_error`)
  - Set/Get control (`flux_msg_set_control`, `flux_msg_get_control`)
  - Fprint (`flux_msg_fprint`, `flux_msg_fprint_ts`)
  - Routes (`flux_msg_route` family of functions)
- KVS (`flux_kvs` family of functions and `flux_kvs_txn_t`)
  - Get/Wait Version (`flux_kvs_get_version`, `flux_kvs_wait_version`)
  - Drop Cache (`flux_kvs_dropcache`)
  - KVS Fencing (`flux_kvs_fence`)
  - Get rootref (`flux_kvs_commit_get_rootref`)
  - KVS Dir (`flux_kvsdir_t` and associated functions)
  - Kvs Getroot Blobref and Treeobj (`flux_kvs_getroot_get_treeobj`, `flux_kvs_getroot_get_blobref`)
  - KVS Lookup treeobj and dir (`flux_kvs_lookup_get_treeobj`, `flux_kvs_lookup_get_dir`)
  - KVS Transaction put_treeobj, clear, and is_empty(`flux_kvs_txn_put_treeobj`, `flux_kvs_txn_clear`, `flux_kvs_txn_is_empty`)
  - Everything related to `treeobj`
- Watchers (`flux_watcher_t`)
  - Handle watcher get Flux (`flux_handle_watcher_get_flux`)
  - FD Watcher get FD (`flux_fd_watcher_get_fd`)
  - Timer watcher
  - Periodic watcher
  - Prepare/Check/Idle watcher
  - Child watcher
  - Signal watcher
  - Stat watcher
  - Create custom watcher (`flux_watcher_create`)
  - Get watcher data (`flux_watcher_get_data`)
  - Get watcher ops (`flux_watcher_get_ops`)
- Configuration (`flux_conf_t`)
- Message List (`flux_msglist` family of functions)
- Event (`flux_event` family of functions)
- Treeobj
- Command (`flux_cmd` family of functions)
- Host Map, Id Map
- Service registration (`flux_service` family of functions)
- Subprocess (`flux_subprocess` family of functions)
- `flux_sync_create`
- Version (`flux_core_version`, `flux_core_version_string`)