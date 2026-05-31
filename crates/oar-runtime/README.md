# oar-runtime

`oar-runtime` 提供最小化的 Backend Runtime 壳：周期触发 one-shot tick，并在 `CancellationToken` 取消后停止。

当前默认适配 `oar-core` 的 `PostgresTenantMaintenanceWorker::run_once()`，不引入 daemon 到 `oar-core`。

Runtime report 只保留成功/失败计数和最后一次 tick，避免常驻进程累积无界历史；错误日志只记录 safe error 分类。

`TenantMaintenanceRuntimeRegistry` 提供多租户 runtime 前置边界：按 tick 周期顺序触发多个 named tenant tick，单租户失败不会阻断后续租户；registry report 只保存 completed round 计数、每租户成功/失败计数和 last tick。真实 adapter builder、tenant discovery、backoff 和生产监控仍在外层接入。

`DiscoveringTenantMaintenanceRuntime` 用于后续生产 daemon 接入：每轮 tick 重新执行 tenant discovery，再按当前 tenant id 构造 fresh tick 并运行一轮。这样不会缓存启动时租户列表，也不会把构造时的绝对时间窗口带进长跑进程。它仍然只接受 generic discovery/factory/tick，不读取 env、不连接 Feishu、不 drain outbox。

生产 facade 启动 daemon 前必须显式配置真实 audit outbox sink。`WebhookAuditOutboxSink` 位于 adapter 层，用于后续生产组装；`NoopAuditOutboxSink` 只能用于测试或本地 contract 验证，不能作为 tenant maintenance daemon 的默认装配。

新增 `TenantMaintenanceRuntimeRegistryBuilder` 作为可测试构建 API：

- `RuntimeTenantDiscovery`：负责发现 tenant id 列表，错误通过 `safe_error` 进入 build error。
- `RuntimeTenantTickFactory`：按 tenant id 构造具体 `RuntimeTick`。
- `StaticRuntimeTenantDiscovery`：配置驱动/静态 tenant discovery 实现，便于测试与后续配置接入。
- `PostgresRuntimeTenantDiscovery`：基于 `PostgresTenantRepository::list_active_ids` 查询 Postgres active tenant 列表（按 id 升序），并把 discovery 错误映射为安全分类字符串，不透出 SQL/raw error。

builder 会在构建阶段执行安全校验：zero interval、空列表、空白 tenant id、重复 tenant id、discovery/factory 失败均返回显式安全错误；tenant id 会在入 registry 前做 trim canonicalization。
