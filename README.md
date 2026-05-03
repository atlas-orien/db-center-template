# db-center-template

这是一个给 AI 使用的 Rust 服务端业务模板。

## License

This project is open source under either the MIT License or the Apache License
2.0, at your option. See [LICENSE](./LICENSE), [LICENSE-MIT](./LICENSE-MIT),
and [LICENSE-APACHE](./LICENSE-APACHE).

用户真正需要做的事情只有三步：

1. 让 AI 先把环境跑起来并测试是否正常
2. 提出业务需求，让 AI 先生成“服务端开发文档”
3. 用户确认文档后，再让 AI 开发代码

## 第零步：先拉取模板

如果你还没有生成项目，可以先让 AI 帮你安装 `Rust`、`cargo-generate`、`Docker`，然后再用模板创建项目。

你可以直接把下面这段话发给 AI：

```text
请先帮我准备这个模板项目的基础环境。

需要你按顺序完成：
1. 检查是否已经安装 Rust
2. 如果没有，安装 Rust
3. 检查是否已经安装 cargo-generate
4. 如果没有，安装 cargo-generate
5. 检查是否已经安装 Docker
6. 如果没有，协助安装 Docker
7. 用下面这个模板生成项目：
   cargo generate --git https://github.com/atlas-form/db-center-template.git --name my-service
8. 进入项目目录
9. 告诉我下一步该做什么

如果中间遇到权限、网络、版本问题，请先处理这些问题，不要跳过。
如果 Docker 拉取镜像失败，请自行处理可用镜像源，不要把镜像源问题直接丢给我。
```

如果你已经有项目目录，可以直接跳到下一步。

## 第一步：先让 AI 安装环境并测试当前项目

生成项目后，先不要直接开发功能。

先让 AI 帮你把环境跑起来，并验证下面几件事：

- Rust 和 cargo-generate 是否可用
- Docker 是否可用
- 配置文件是否正确
- PostgreSQL 是否可用
- `auth` 服务是否可访问
- 如果业务会用 AI/LLM：本地或远程模型服务是否可访问
- 当前项目是否能正常启动
- 管理端接口是否可测试

如果过程中出现报错，优先让 AI 自己排查和修复，不要立刻停下来等用户处理。

你可以直接把下面这段话发给 AI：

```text
请先不要开发业务功能。

先帮我完成环境检查和启动测试：
1. 检查 Rust、cargo-generate、Docker 是否可用，如缺少请协助安装
2. 检查 config/services.toml 是否合理
3. 检查 PostgreSQL 是否可用，如没有则协助启动
4. 检查 auth 服务是否可访问
5. 运行数据库初始化和迁移
6. 启动当前服务
7. 如果 config/services.toml 里配置了 `[[llm]]`，测试模型服务是否可访问
8. 核对 `API_CONTRACTS/` 下的 Markdown 接口文档是否与当前代码一致
9. 告诉我当前环境是否已经可以开始开发

如果中间发现问题，请先修复环境问题，不要直接开始写业务代码。
如果 Docker 拉取镜像失败，请自行处理镜像源问题。
如果出现其它错误，请先自行排查和修复，再告诉我结果。
```

## 如果初始化数据库失败

如果 AI 在本地没法把数据库初始化成功，不需要让用户自己研究数据库安装问题。

这时可以让用户去找技术同事要一个已经可用的数据库连接字符串，然后把这个连接发给 AI。

例如：

```text
postgres://username:password@host:5432/database_name
```

拿到这个链接后，AI 应该自己完成这些事情：

1. 测试这个数据库连接是否可用
2. 如果可用，自动修改 `config/services.toml`
3. 如有需要，自动修改 `.env`
4. 重新执行数据库初始化、迁移和启动测试
5. 告诉用户当前项目是否已经可以正常运行

你可以直接把下面这段话发给 AI：

```text
如果本地数据库初始化失败，我可以提供一个技术同事给我的 PostgreSQL 连接。

请你拿到连接后自己完成以下事情：
1. 先测试这个数据库连接是否可用
2. 如果可用，自动修改当前项目的配置文件
3. 重新执行数据库初始化、迁移和启动测试
4. 如果过程中还有错误，请继续自行排查和修复
5. 最后告诉我项目是否已经可以正常运行

数据库连接如下：
<把数据库连接放这里>
```

## 第二步：用户提出业务需求

环境确认没问题之后，用户只需要描述业务需求。

例如：

```text
我要做一个设备管理功能。

需求：
1. 用户可以创建设备
2. 用户可以查看自己创建的设备
3. 管理员可以查看全部设备
4. 接口需要登录后访问
```

如果需求涉及 AI/LLM，请同时说明：

- 需要文本生成、图片识别，还是其它模型能力
- 期望使用哪个模型服务，例如本地 Ollama 或远程 OpenAI-compatible provider
- 输入来自文本、图片 URL、base64 data URL，还是文件上传
- 是否需要支持多个模型配置

当前模板已经内置 `model-gateway-rs`，并在 `crates/web-server/src/statics/llm_client.rs` 中提供统一 LLM client registry。

配置示例见 `config/services-example.toml`：

```toml
[[llm]]
name = "ollama-gemma4"
base_url = "http://127.0.0.1:11434"
model = "gemma4:26b"
max_tokens = 20000
temperature = 0.2
```

本地 Ollama 图片识别 smoke 测试：

```bash
cargo run -p web-server --example llm_vision_smoke
```

## 第三步：让 AI 先生成服务端开发文档

这一步很重要。

不要让 AI 一上来直接写代码，而是先让它输出“服务端开发文档”，至少包含：

- 业务理解
- 数据表设计
- 字段设计
- 表关系
- 权限设计
- HTTP 接口设计
- 开发顺序

你可以直接把下面这段话发给 AI：

```text
请先不要直接写代码。

先阅读：
1. AI_PROTOCOLS/TABLE_ADDING_PROTOCOL.md
2. 如果涉及登录用户，再阅读 AI_PROTOCOLS/AUTH_INTEGRATION_GUIDE.md
3. 如果涉及 AI/LLM，再阅读 AI_PROTOCOLS/LLM_CLIENT_GUIDE.md

然后根据我的业务需求，先输出一份“服务端开发文档”，内容至少包括：
1. 业务理解
2. 数据表设计
3. 每张表的字段、类型、约束
4. 表之间的关系
5. 哪些字段需要关联当前登录用户 user_id
6. 需要提供的 HTTP 接口
7. 每个接口的权限要求
8. 如果涉及 AI/LLM：模型配置名称、输入输出格式、失败处理和验证方式
9. 开发顺序

不要开始写代码，先等我确认。
```

## 第四步：用户确认后再开发

用户确认服务端开发文档后，再让 AI 正式开发。

你可以这样说：

```text
现在开始开发。

请严格按照已经确认的服务端开发文档执行。
同时遵守 AI_PROTOCOLS/TABLE_ADDING_PROTOCOL.md。
如果涉及登录用户，同时遵守 AI_PROTOCOLS/AUTH_INTEGRATION_GUIDE.md。
如果涉及 AI/LLM，同时遵守 AI_PROTOCOLS/LLM_CLIENT_GUIDE.md。

按以下顺序完成：
1. migration
2. entity
3. repo
4. service
5. web-server

如果发现设计需要变更，请先停下来告诉我，不要擅自修改。
```

## 建议阅读顺序

1. [user_docs/core_design.md](./user_docs/core_design.md)
2. [user_docs/requirement_to_backend_doc.md](./user_docs/requirement_to_backend_doc.md)
3. [user_docs/requirement_template.md](./user_docs/requirement_template.md)
4. [user_docs/environment_setup_with_ai.md](./user_docs/environment_setup_with_ai.md)
5. [user_docs/how_to_use_ai.md](./user_docs/how_to_use_ai.md)

## 给 AI 的入口文档

如果你希望 AI 按统一流程工作，建议它先阅读：

1. [AI_PROTOCOLS/AI_WORKFLOW.md](./AI_PROTOCOLS/AI_WORKFLOW.md)
2. [AI_PROTOCOLS/TABLE_ADDING_PROTOCOL.md](./AI_PROTOCOLS/TABLE_ADDING_PROTOCOL.md)
3. 如果涉及登录用户，再阅读 [AI_PROTOCOLS/AUTH_INTEGRATION_GUIDE.md](./AI_PROTOCOLS/AUTH_INTEGRATION_GUIDE.md)
4. 如果涉及 AI/LLM，再阅读 [AI_PROTOCOLS/LLM_CLIENT_GUIDE.md](./AI_PROTOCOLS/LLM_CLIENT_GUIDE.md)

## 说明

根目录这个 `README.md` 只用于模板仓库本身。

生成项目时，`cargo-generate` 会忽略它；生成后的主要说明文档在 `user_docs/` 目录中。
