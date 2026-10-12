# Auth 集成协议 (AI 专用)

> 本文档描述业务服务如何接入统一认证服务 `auth`。
> 目标是让 AI 在实现业务接口时，只依赖 JWT 中的 `user_id` 即可完成大多数“当前用户”场景。

---

## 一、定位

本模板中的业务服务 **不负责**：

- 用户注册
- 登录
- 刷新 Token
- JWT 签发
- 用户资料管理

这些能力统一由外部 `auth` 服务提供。

业务服务只负责两件事：

1. 验证 `auth` 签发的 access token
2. 从 JWT 中提取当前用户 `user_id`，用于业务权限与数据归属判断

---

## 二、与 auth 的集成方式

### 1. 远程获取 JWT 验签配置

业务服务启动时，必须调用 `auth` 的内部接口：

`GET /internal/jwt_verify_config`

请求头：

- Header 名称来自 `config/services.toml` 中的 `jwt_verify.header`
- Header 值来自 `config/services.toml` 中的 `jwt_verify.token`

返回值中会包含：

- `public_key_pem`
- `issuer`
- `audience`

业务服务使用这 3 个字段初始化 `toolcraft_jwt::VerifyJwt`。

### 2. 当前模板中的配置格式

```toml
[jwt_verify]
url = "http://127.0.0.1:29001/internal/jwt_verify_config"
header = "x-internal-token"
token = "dev-internal-token-change-me"
```

### 3. 认证中间件的用法

受保护路由必须使用：

```rust
route_layer(from_fn(auth::<VerifyJwt>))
```

当前用户在 Handler 中通过以下方式获取：

```rust
Extension(auth_user): Extension<AuthUser>
```

当前阶段，业务代码默认只使用：

```rust
auth_user.user_id
```

---

## 三、AI 在实现业务时必须遵守的规则

### 用户资料与文件访问契约

Auth 用户资料的 `avatar` 是包含 `object_key` 的对象或 `null`，不是公开 URL。
当前后端 `AuthUserResponse` 只消费 ID 和名称字段，Serde 忽略头像等未消费字段，
无需为头像读取增加后端存储或接口。前端向 File Service 的 `/file/sign/access`
提交 `bucket_id=avatar` 和 object key，由 File Service 按读取策略返回公开地址或私有签名地址。
头像上传后向 Auth 保存 object key；S3 bucket、域名和凭据继续由 File Service 管理。

### 允许

- 使用 JWT 中的 `user_id` 作为“当前登录用户”
- 在业务表中保存 `created_by`、`owner_user_id`、`updated_by` 等字段
- 在 `service` 层按 `user_id` 做权限校验或数据过滤
- 在 `web-server` 层为需要登录的接口挂鉴权中间件

### 禁止

- 在业务服务里重复实现注册、登录、刷新 token
- 在业务服务中重新维护一套 users 表，除非需求明确要求做用户快照或冗余映射
- 假设 JWT 中有除 `user_id` 之外的稳定业务字段
- 为了获取当前用户信息而直接读取 `auth` 的数据库

---

## 四、推荐模式

### 模式 1：只需要当前用户 ID

这是默认模式，也是最常见模式。

适用场景：

- “查询我的订单”
- “创建我的申请单”
- “只能修改自己创建的数据”

实现方式：

1. 路由挂 JWT 中间件
2. Handler 提取 `auth_user.user_id`
3. 把 `user_id` 传给 `service` 层

### 模式 2：确实需要 auth 的额外用户资料

只有在明确需要时，才通过 HTTP 调用 `auth` 服务提供的外部或内部接口。

默认不要这样做，除非需求中明确出现：

- 展示用户名
- 展示头像
- 根据 auth 用户状态做跨服务校验

---

## 五、模板中的示例路由

本模板提供了一个最小示例：

- `GET /example/me`

用途：

- 演示如何挂 JWT 校验
- 演示如何在 Handler 中获取 `user_id`

返回结构：

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "user_id": "xxx"
  }
}
```

---

## 六、给 AI 的执行建议

当用户要求“这个业务接口需要登录后访问”时，默认按以下顺序处理：

1. 在 `routes` 中给该路由挂 JWT 中间件
2. 在 `handlers` 中提取 `AuthUser`
3. 将 `auth_user.user_id` 映射到 `service` 请求 DTO
4. 在 `service` 中完成归属校验、权限校验或数据过滤

如果用户没有明确要求，不要把 `auth` 的用户字段复制到业务库中。
