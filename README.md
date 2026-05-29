# OAR

> 面向飞书企业租户的 **OKR 复盘驾驶舱**：每周发现执行风险，汇总证据，起草行动建议，并且只在用户确认后安全写回飞书。

OAR 不是通用 OKR SaaS，不替代飞书 OKR，也不是绩效评价系统。第一版只做一件窄而高频的事：让 manager / PMO 每周用一个可靠的 Review Inbox 运营已经存在的 OKR。

| 维度 | 当前判断 |
| --- | --- |
| 当前阶段 | Phase 0.5 已完成；Phase 0.6 进行中 |
| 核心入口 | Weekly Review Inbox，不是聊天框或大屏仪表盘 |
| 权威数据源 | Lark / Feishu 负责原始租户数据；OAR 负责复盘、动作、审计和决策 |
| 生产集成 | Rust 原生 OpenAPI adapter（`crates/oar-lark-adapter`） |
| 安全原则 | 读先行、写前 dry-run、执行前人工确认、写后 append-only audit |

## 产品切口

很多团队已经在飞书里维护 OKR，真正痛的是每周持续运营它：

- KR 长期没人更新，风险暴露太晚。
- 证据分散在 OKR、Docs、Tasks、Meetings、Minutes、Calendar 和 IM 中。
- manager / PMO 需要手动整理周报、催进展、约同步、写评论。
- 企业不能接受黑盒自动执行，需要权限继承、证据链、人工确认和审计。

OAR 的第一性判断：

> OKR 的主要机会不在“帮人写目标”，而在“帮人每周运营目标执行风险”。

用户每周应该看到：

| 区域 | 作用 |
| --- | --- |
| 风险队列 | 展示长期未更新、低进度、缺证据或缺 owner 更新的 KR |
| 证据链 | 给出来源、摘要、引用、hash 和可见范围 |
| 建议动作 | 起草 progress、评论、提醒、任务或会议草稿 |
| 人工门禁 | 支持确认、编辑后确认、拒绝 |
| 审计时间线 | 记录谁基于什么证据确认了什么动作，结果如何 |

## 设计原则

| 原则 | 含义 |
| --- | --- |
| 高频复盘收件箱 | 不帮用户“创建 OKR”，而是帮助用户每周清空 OKR 执行风险和待确认动作 |
| 可解释证据链 | 每条风险警报和改动建议都必须绑定脱敏证据引用与 hash |
| 严格人机门禁 | L1-L3 可以自动准备；L4 写回必须由人类用户显式确认；第一阶段不做 L5 完全自主执行 |
| 受控 A2A 路线 | Phase 3 以后才开放只读 `A2A Server`；外部智能体即使能提交建议，写回仍由 OAR 人工门禁收口 |

## 主链路

```mermaid
flowchart LR
    subgraph Lark["Feishu / Lark 权威数据源"]
        OKR["OKR"]
        EvidenceSource["Docs / Tasks / Meetings / IM"]
    end

    subgraph Core["OAR Rust Core"]
        Sync["同步证据<br/>summary / reference / hash"]
        Risk["风险检测<br/>stale / low progress / missing evidence"]
        Proposed["ProposedAction<br/>建议动作 + evidence chain"]
        Ledger["OperationLedger<br/>幂等执行账本"]
        Audit["AuditEvent / Outbox<br/>append-only 审计"]
    end

    subgraph Inbox["Review Inbox"]
        Queue["Weekly Queue<br/>风险与待确认动作"]
        Gate{"Human Gate<br/>确认 / 编辑后确认 / 拒绝"}
    end

    subgraph Writeback["受控写回"]
        DryRun["dry-run<br/>预演影响范围"]
        Adapter["LarkAdapter<br/>allowlist OpenAPI"]
    end

    OKR --> Sync
    EvidenceSource --> Sync
    Sync --> Risk --> Proposed --> Queue --> Gate
    Gate -->|拒绝| Audit
    Gate -->|确认或编辑后确认| Ledger --> DryRun --> Adapter
    Adapter -->|progress / comment| OKR
    Adapter --> Audit
    Audit -.审计时间线.-> Queue

    classDef source fill:#eef6ff,stroke:#4078c0,color:#111;
    classDef core fill:#f7f7f7,stroke:#8a8f98,color:#111;
    classDef inbox fill:#fff7e6,stroke:#b7791f,color:#111;
    classDef guard fill:#fff1f2,stroke:#c53030,color:#111;
    classDef audit fill:#eefaf0,stroke:#2f855a,color:#111;
    class OKR,EvidenceSource source;
    class Sync,Risk,Proposed,Ledger,DryRun,Adapter core;
    class Queue inbox;
    class Gate guard;
    class Audit audit;
```

## 当前状态

| 阶段 | 状态 | 结论 |
| --- | --- | --- |
| Phase 0.5 | 已完成 | `lark-okr` 已验证本地 OKR 读取、progress 创建 / 更新验证和 fixture 回归；progress 删除仍保持 dry-run |
| Phase 0.6 | 进行中 | identity、token grant、device session、operation ledger、audit 和 Postgres schema contract 已进入过渡态验证 |
| 生产闭环 | 未完成 | 真实 Feishu live network、后台 scheduler/daemon、revoke/reauth、多端同步仍需继续验证 |

已具备的工程基础：

- `oar-core` 已包含 identity、token grant、device session、operation ledger、audit 和 Postgres schema contract。
- token refresh service、Postgres Recorder、audit 映射和显式 `run_once` refresh sweep 已完成部分验证。
- 生产飞书集成主路径已从 CLI 验证收敛到 Rust 原生 OpenAPI adapter。
- macOS client 默认连接本地 OAR backend；`oar-http-facade` 已提供前端合同的安全本地 route 壳。

## 安全模型

OAR 默认保守，所有真实写回必须走同一条受控链路：

```text
ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent
```

关键约束：

- 先读后写，写前 dry-run，执行前人工确认。
- 所有写回必须来自 `ConfirmedAction`。
- 业务代码只能通过 `LarkAdapter` / `PlatformAdapter` 或明确设计过的 adapter 层调用飞书。
- `OperationLedger` 保证同一个确认动作只执行一次。
- `AuditEvent` 记录 actor、scope、target、before/after 摘要和执行结果。
- access token、refresh token、authorization code、raw CLI stdout/stderr、encrypted blob 和 fingerprint 不得出现在日志、审计 payload 或用户可见错误里。

## 技术架构

| 层 | 选择 | 说明 |
| --- | --- | --- |
| macOS client | SwiftUI + AppKit bridge | Review Inbox 主体验 |
| iOS companion | SwiftUI | 轻量查看、提醒、确认入口 |
| Core / backend | Rust | domain、storage、execution、audit、sync contract |
| Feishu integration | `crates/oar-lark-adapter` | Rust 原生 OpenAPI runtime adapter |
| HTTP facade | `crates/oar-http-facade` | macOS client 的本地 HTTP contract 壳 |
| Backend runtime | `crates/oar-runtime` | 周期触发 tenant maintenance one-shot tick，不下沉 daemon 到 core |
| Storage | Postgres + pgvector | relational + vector，避免引入 graph DB |
| CLI | `lark-okr` | 仅用于验证、fixtures 和 regression tests |

目录概览：

```text
.
├── apps/oar/                    # SwiftUI macOS client：OAR 复盘收件箱入口
├── crates/oar-core/             # Rust core：domain、storage、execution、audit
├── crates/oar-http-facade/      # 本地 HTTP facade：前端合同与安全占位 route
├── crates/oar-lark-adapter/     # Rust 原生飞书 OpenAPI runtime adapter
├── crates/oar-runtime/          # 后台 runtime 壳：interval + cancellation
├── docker/                      # Dockerfile 与 compose 编排
├── docs/project-overview.md     # 项目定位、路线图、核心决策
├── docs/review-inbox.md         # MVP PRD、复盘收件箱体验和工作流
├── docs/system-architecture.md  # Rust core、storage、LarkAdapter、scheduler 架构
├── docs/feishu-integration.md   # Phase 0.5 飞书 / Lark CLI 验证结论
├── docs/identity-auth-sync.md   # Phase 0.6 identity、auth refresh、device sync 验证
├── docs/execution-audit.md      # ConfirmedAction、OperationLedger、AuditEvent 和权限边界
├── docs/agent-capabilities-feishu-permissions.md # agent capability、action_type、scope 和门禁矩阵
├── docs/memory-evidence.md      # 证据链、三层记忆和检索设计
├── docs/validation-plan.md      # 总体验证计划、阶段门和停止标准
├── docs/reference/              # 外部参考、竞品、依赖雷达和技术资料
├── .env.example                 # 后端运行时 env 模板
├── Cargo.toml                   # Rust workspace
└── AGENTS.md                    # 项目级 AI agent 工作约束
```

`oar-core` 关键位置：

| 路径 | 作用 |
| --- | --- |
| `crates/oar-core/src/domain/identity.rs` | Tenant、WorkspaceUser、Lark identity、token grant 和 actor 模型 |
| `crates/oar-core/src/domain/device_sync.rs` | Device session 与 sync cursor 语义 |
| `crates/oar-core/src/domain/token_refresh/` | Token refresh 类型、决策、bridge 和 service |
| `crates/oar-core/src/action/` | ConfirmedAction、OperationLedger、AuditEvent、ExecutionPolicy |
| `crates/oar-core/src/lark/` | Lark adapter、parser、fixtures 和 auth refresh 边界 |
| `crates/oar-core/src/storage/postgres/` | SQL contract、Postgres repository、Recorder、outbox worker |
| `crates/oar-core/migrations/` | Phase 0.6 Postgres migration 草案 |

模块路径说明：`domain::token_refresh` 和 `lark::auth` 不再提供 root facade re-export。新 Rust 代码应使用真实子模块路径，例如 `domain::token_refresh::{bridge,decision,service,types}` 和 `lark::auth::{adapter,parser,types}`。

## 开发验证

当前 workspace 包含 `oar-core`、`oar-http-facade`、`oar-lark-adapter` 和 `oar-runtime`。`oar-core` 保持 core/storage/contracts 边界，不直接依赖 HTTP runtime、CLI 或 SDK；生产飞书集成固定收敛在 `oar-lark-adapter`，常驻调度语义收敛在 `oar-runtime`。

常用检查：

```bash
cargo fmt --check
cargo check --workspace --tests
cargo test -p oar-core
cargo test -p oar-http-facade
cargo test -p oar-lark-adapter
cargo test -p oar-runtime
cargo test -p oar-core --features postgres
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p oar-core --all-targets -- -D warnings
cargo clippy -p oar-core --features postgres --all-targets -- -D warnings
```

Postgres live tests 由 `DATABASE_URL` 控制。未设置时，默认测试仍会覆盖 domain / in-memory contract，以及 SQL text / schema contract。

```bash
DATABASE_URL=postgres://... cargo test -p oar-core --features postgres --test postgres_live_repository
```

OAR macOS 默认连接本地 backend facade：

```bash
# terminal 1
cargo run -p oar-http-facade

# terminal 2
cd apps/oar
swift run
```

当前 `oar-http-facade` 已能在配置飞书 OAuth 凭证后创建真实授权 URL，并通过
`GET /auth/feishu/callback` 在服务端用授权码换取用户凭证、读取安全用户信息，再
返回 OAR 自己的会话字段。配置 `DATABASE_URL` 和 grant key 后，callback 成功会将用户绑定 OAuth grant
加密落库为 `TokenGrant`，未配置数据库时保留本地内存登录行为；真实 Review Inbox 数据仍是后续工作；
`GET /review-inbox/snapshot` 目前返回空快照，decision 写路径明确返回不支持。前端期望的 HTTP endpoint 记录在
[`apps/oar/README.md`](apps/oar/README.md)。

Agent 模型请求只从后端发出。macOS 前端通过 `POST /agent/stream` 发送会话上下文和消息，
并携带 OAR session bearer；后端优先使用用户级 BYOK 设置，没有用户设置时再按
`OAR_AGENT_PROVIDER` 选择 OpenAI-compatible 或 Anthropic 默认 adapter。前端设置页只提交
`baseURL` / `apiKey` 给后端做协议和模型检测，API key 会用 `OAR_GRANT_KEY_*` 加密落库，
不会让前端直连模型服务。本地验证默认 Agent 需要额外配置：

```bash
OAR_AGENT_PROVIDER=openai-compatible
OAR_AGENT_OPENAI_BASE_URL=https://api.openai.com/v1
OAR_AGENT_OPENAI_API_KEY=...
OAR_AGENT_OPENAI_MODEL=gpt-4.1

# 或：
OAR_AGENT_PROVIDER=anthropic
OAR_AGENT_ANTHROPIC_API_KEY=...
OAR_AGENT_ANTHROPIC_MODEL=claude-sonnet-4-5
```

后端 Docker 运行时可以用环境变量覆盖监听地址。默认本地开发绑定
`127.0.0.1:8080`，容器内运行绑定 `0.0.0.0:8080`：

```bash
OAR_HTTP_BIND_ADDR=0.0.0.0:8080 cargo run -p oar-http-facade
```

最小 Docker 打包入口：

```bash
docker build -f docker/backend.Dockerfile -t oar-backend .
docker run --rm -p 8080:8080 oar-backend
```

本地开发 compose 会启动 backend 和本地 Postgres volume：

```bash
docker compose -f docker/compose.dev.yml up --build
```

`compose.dev.yml` 会在全新的 Postgres volume 首次初始化时自动加载 `crates/oar-core/migrations/`
里的 schema。若 volume 已经存在但表还没建，可以手动补一次：

```bash
docker exec -i oar-postgres-1 psql -U oar -d oar -v ON_ERROR_STOP=1 < crates/oar-core/migrations/0001_phase_0_6_identity_action_audit.sql
docker exec -i oar-postgres-1 psql -U oar -d oar -v ON_ERROR_STOP=1 < crates/oar-core/migrations/0002_review_inbox_domain.sql
docker exec -i oar-postgres-1 psql -U oar -d oar -v ON_ERROR_STOP=1 < crates/oar-core/migrations/0003_agent_model_settings.sql
```

生产 / 云端 compose 只启动 backend，必须显式提供外部 `DATABASE_URL`：

```bash
DATABASE_URL=postgres://... docker compose -f docker/compose.yml up --build
```

后端 env 模板见 [`.env.example`](.env.example)。本地 `cargo run -p oar-http-facade`
会自动加载仓库根目录的 `.env`，可以用 `cp .env.example .env` 后填入真实飞书凭证；
如果暂时只测扫码、不落库，可删除本地 `.env` 里的 `DATABASE_URL`；如果要用本机端口连接 compose Postgres，
则把本地 `.env` 里的 `DATABASE_URL` 改成 `postgres://oar:oar@127.0.0.1:5432/oar`。
`.env` 已在 `.gitignore` 中排除，不要提交真实 secret。`docker/compose.dev.yml`
会从 shell 或本地 `.env` 读取可选配置，并在未提供 `DATABASE_URL` 时启动本地 Postgres volume。
生产/云端部署使用默认 `docker/compose.yml`，必须显式注入 `DATABASE_URL`，不会静默降级成本地存储。
飞书扫码登录需要额外配置 `OAR_FEISHU_APP_ID`、`OAR_FEISHU_APP_SECRET` 和
`OAR_FEISHU_REDIRECT_URI`；其中 redirect URI 必须在飞书开发者后台安全设置中登记，
且移动端扫码时需要是手机可访问的公网地址。飞书 app secret、token 和绕过人工确认 / ledger 的开关不放入 Dockerfile。
未设置 `OAR_FEISHU_AUTH_SCOPE` 时，代码内置默认授权会请求 OAR 已声明用户级能力所需的
Feishu scopes，包括 OKR 读写、OKR review/setting 读取、calendar free-busy、task 读写等。
`authen/v1/user_info` 获取基础身份不需要额外应用权限，`offline_access` 用于让飞书返回 refresh token，
支持 OAR 加密落库 `TokenGrant` 并后续刷新。飞书开发者后台必须先开启默认授权包含的对应权限。
设置变更后必须让用户重新用 OAR 扫码授权；旧 `TokenGrant` 不会因为后台新增 app scope 自动获得 OAuth grant scope。
飞书 app scope、用户 OAuth grant scope 和 OAR allowlist 是三层不同门禁；默认授权变宽不代表绕过执行边界，
写操作仍然必须经过 dry-run、人工确认、`OperationLedger` 和 `AuditEvent`。
本地开发可临时设置 `OAR_ALLOW_EPHEMERAL_GRANT_KEY=true` 让 auth refresh 配置自动生成一次性内存
grant key；生产环境不要打开，必须注入稳定的 `OAR_GRANT_KEY_ID` / `OAR_GRANT_KEY_HEX`。

飞书应用凭证模型：

| 场景 | `OAR_FEISHU_APP_ID` / `OAR_FEISHU_APP_SECRET` 来源 | 用户扫码后的身份 |
| --- | --- | --- |
| OAR 官方 SaaS | OAR 官方在飞书开放平台创建并发布的应用 | 每个租户用户授权后生成自己的 `user_access_token` / `TokenGrant` |
| 企业私有化部署 | 企业管理员在自己的飞书开发者后台创建自建应用 | 该企业内部用户授权后生成各自的 `TokenGrant` |
| 本地开发 | 开发者在测试租户创建一个开发应用 | 测试用户授权后生成本地测试 `TokenGrant` |

因此 `App ID` / `App Secret` 可以按部署固定，但不能硬编码到客户端，也不等于“所有用户共用一个机器人”。
机器人/应用身份用于系统通知等 `bot_actor` 场景；用户扫码授权产生的是按 `tenant_key + open_id` 绑定的
`user_delegated` grant，后续读写仍受飞书应用 scope、用户资源权限和 OAR allowlist 共同限制。

## 文档地图

建议按这个顺序读：

1. [项目概览](docs/project-overview.md)：定位、路线图、关键风险和阶段状态。
2. [复盘收件箱](docs/review-inbox.md)：需求、验收标准与工作流。
3. [系统架构总览](docs/system-architecture.md)：Swift/Rust/LarkAdapter/storage 设计。
4. [执行与审计边界](docs/execution-audit.md)：执行边界和数据处理原则。
5. [Agent capabilities 与飞书权限矩阵](docs/agent-capabilities-feishu-permissions.md)：能力、adapter/action_type、飞书 scope、风险等级和 dry-run/确认/audit 要求。
6. [验证计划](docs/validation-plan.md)：阶段门、实验和停止标准。
7. [阶段 0.5 飞书集成验证](docs/feishu-integration.md)：OKR CLI 实测结论。
8. [阶段 0.6 身份与同步验证](docs/identity-auth-sync.md)：identity、token refresh、sync、idempotency 和 audit 进展。

完整文档目录见 [docs/README.md](docs/README.md)。

## 近期工作

下一步是把 Phase 0.6 已验证的组件推进到真实生产路径：

- [ ] 接入真实 `AuthAdapter` / client，验证安全解析到 `RefreshOutcome`
- [ ] 在现有显式 `run_once` sweep 边界上接入 scheduler/daemon 触发
- [ ] 扩展 Postgres Recorder / audit 测试，覆盖 retry、timeout、stale fingerprint、revoke 和 reauth
- [ ] 证明 macOS、iOS 和飞书卡片入口能看到同一个后端动作状态
- [ ] 用真实团队跑复盘收件箱原型，验证每周使用习惯是否成立
