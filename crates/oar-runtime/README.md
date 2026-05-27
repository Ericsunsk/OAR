# oar-runtime

`oar-runtime` 提供最小化的 Backend Runtime 壳：周期触发 one-shot tick，并在 `CancellationToken` 取消后停止。

当前默认适配 `oar-core` 的 `PostgresTenantMaintenanceWorker::run_once()`，不引入 daemon 到 `oar-core`。

Runtime report 只保留成功/失败计数和最后一次 tick，避免常驻进程累积无界历史；错误日志只记录 safe error 分类。
