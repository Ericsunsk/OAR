# OAR 推荐依赖库与 Crates 建议清单（技术雷达）

> 本文是候选依赖技术雷达，不是 Phase 0.6 架构决策或生产采纳承诺。若与 `docs/project-overview.md`、`docs/system-architecture.md`、`docs/validation-plan.md` 等 source-of-truth 文档冲突，以 source-of-truth 为准。尤其是：是否“已接入生产”只能以验证报告和代码测试结论为准。

为了防止在后续开发（Phase 0.7 证据链、A2A 协议等）中重复造轮子，并确保系统的高安全性、原生性能与隐私隔离，本指南对未来开发切片中引入的候选依赖进行边界说明：`oar-core` 维持 pure core/storage/contracts，network/runtime/gateway 依赖放在外层（例如 `oar-lark-adapter` 或后续 runtime crate）。

---

## 1. 凭证防泄与信封加密（已部分接入）

* **`secrecy`**（已在 `oar-lark-adapter` 中使用）
  * **现状**：`oar-lark-adapter` 中的 `SecretString` 底层已包裹 `secrecy::SecretString` 投入使用，非完全自研。
  * **后续建议**：持续在核心层和传输边界间维持 `SecretString` 包装，利用其 **Zeroing-on-Drop** 特性保障明文 Token 在被 Drop 后自动在内存中清零，防止 coredump 泄漏。
* **`aes-gcm`**（已在 `oar-lark-adapter` 中使用）
  * **现状**：底层已引入，用于 `crypto` 密算支持。
  * **后续建议**：在 Phase 0.6 晚期打通真实落库时，规范采用 AES-256-GCM 对 `encrypted_oauth_grant` 进行高强度的信封加密。

---

## 2. 证据链同步与文档文本解析（Phase 0.7 规划中）

* **`pdf-extract`** 或 **`lopdf`**
  * **定位**：纯 Rust 原生的 PDF 文本提取器。
  * **后续建议**：用于在后端服务进程内秒级将 PDF 二进制数据流转化为纯文本，避免拉起外部命令行子进程，降低运行时 CPU 开销。
* **`html2md`**
  * **定位**：HTML 到 Markdown 渲染器。
  * **后续建议**：用于在通过飞书 API 导出云文档/Wiki时，将其自动渲染为 Markdown 文本，以节省大模型的 Context 长度与 Token 费用。

---

## 3. 语义记忆与本地向量引擎（Memory Layer 规划中）

* **`fastembed`**（本地 ONNX Embedding 引擎）
  * **[!IMPORTANT] 边界定义**：
    由于 `fastembed` 需要引入 ONNX 运行时和 Model Artifacts 静态模型文件的分发管理，**绝对不能直接侵入 `oar-core`**。在 Phase 0.7 开展前，必须在外部进行独立的 POC 验证。
* **`pgvector` 向量集成**
  * **[!IMPORTANT] 严谨表述**：
    在 Postgres 向量存储对接上，技术路线应采用 **“`pgvector` crate + `sqlx` integration”** 模式进行原生集成，而不是依赖 sqlx 自身的 feature 属性。并在 SQL 查询时，挂载 `WHERE tenant_id = ?` 锁死多租户隔离界限。

---

## 4. 接口重试与限流退避重试（7x24 高可用保障）

* **`backon`**
  * **[!IMPORTANT] 架构红线**：
    `backon` 或其他重试库仅作为技术候选，**切忌将其简化为“一行代码解决高可用”**。生产级的重试和退避必须配齐：
    1. **Retry Budget**（重试预算限制，防止级联雪崩）。
    2. **Idempotency Keys**（幂等键强校验，确保 side-effect 不重复执行）。
    3. **Error Classification**（精准的错误分类，只对可恢复的 transient error 进行重试）。
    4. **Rate Limit Backoff**（飞书 HTTP 429 频控退避机制）。
    5. **Audit Observability**（每一次重试和退避必须写入审计日志，保证可观测性）。

---

## 5. Webhook签名防伪与常驻网络服务（安全对接网关）

* **`sha2`** 和 **`hmac`**（已在 `oar-lark-adapter` 中使用）
  * **[!IMPORTANT] 边界定义**：
    飞书事件订阅和消息卡片回调的加密和签名流程极其繁琐。**先不要把 `X-Lark-Signature` 强行写死为最终唯一的契约**。后续应当按照飞书最新的官方安全文档，重新核对 Header 集合、签名串提取规则以及 Encrypt Key 解密回调流程，确保安全防护不留盲区。
* **`axum`**
  * **[!IMPORTANT] 架构红线**：
    `axum` 作为 Web 框架，**明确属于外层的 `runtime/gateway` 容器，严禁下沉污染 `oar-core`**。`oar-core` 必须保持纯粹的 Domain Crate 边界，完全与网络协议解耦。
