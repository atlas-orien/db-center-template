use std::collections::{BTreeSet, HashMap, HashSet};

use db_core::{
    DbContext, PaginatedResponse,
    error::{BIZ_INTERNAL_ERROR, BizError, BizResult},
};
use error_code::{admin as admin_error, app as app_error};
use repo::table::{
    app_permissions::{Permission, PermissionService},
    app_role_permissions::{CreateRolePermission, RolePermissionService},
    app_roles::{CreateRole, Role, RoleService},
    app_user_roles::{CreateUserRole, UserRoleService},
    app_users::{
        AppUser, AppUserFilter, AppUserService, AppUserStatus, CreateAppUser, UpdateAppUser,
    },
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    api::admin::AdminApi,
    dto::app::{
        AppUserMetricsResponse, AppUserResponse, AppUserSortBy, CreateRoleRequest,
        CurrentUserPermissionsResponse, ListAppUsersRequest, PermissionTreeNode,
        RegisterAppUserRequest, RolePermissionTreeNode, RoleResponse, SortOrder,
        UpdateAppUserRequest, UpdateRolePermissionsRequest, UpdateUserRolesRequest,
        UserRoleOptionResponse,
    },
};

const DEFAULT_APP_ROLE_NAME: &str = "免费";
const DEFAULT_APP_ROLE_CODE: &str = "free";
const PERM_APP_USERS: &str = "accounts:app_users";
const PERM_APP_ROLES: &str = "access_control:app_roles";
const PERM_APP_ROLE_PERMISSIONS: &str = "access_control:app_role_permissions";

pub struct AppApi {
    db: DbContext,
    admin_api: AdminApi,
    app_user_svc: AppUserService,
    role_svc: RoleService,
    permission_svc: PermissionService,
    user_role_svc: UserRoleService,
    role_permission_svc: RolePermissionService,
}

impl AppApi {
    pub fn new(db: DbContext) -> Self {
        Self {
            db: db.clone(),
            admin_api: AdminApi::new(db.clone()),
            app_user_svc: AppUserService::new(db.clone()),
            role_svc: RoleService::new(db.clone()),
            permission_svc: PermissionService::new(db.clone()),
            user_role_svc: UserRoleService::new(db.clone()),
            role_permission_svc: RolePermissionService::new(db),
        }
    }

    pub async fn register_app_user(
        &self,
        req: RegisterAppUserRequest,
    ) -> BizResult<AppUserResponse> {
        let user_id = parse_user_id(&req.user_id)?;
        let tx = self.db.begin().await?;
        let result = AppApi::new(tx.clone())
            .register_app_user_in_tx(user_id, req)
            .await;
        let app_user = commit_or_rollback(tx, result).await?;

        let roles = self.list_roles_by_user_id(user_id).await?;
        Ok(Self::map_app_user(app_user, roles))
    }

    pub async fn list_app_users(
        &self,
        current_admin_user_id: String,
        req: ListAppUsersRequest,
    ) -> BizResult<PaginatedResponse<AppUserResponse>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let keyword = normalize_optional_string(req.keyword);
        let keyword_user_id = keyword
            .as_deref()
            .and_then(|keyword| Uuid::parse_str(keyword).ok());
        let app_users = self
            .app_user_svc
            .list_paginated(
                AppUserFilter {
                    keyword,
                    keyword_user_id,
                    status: req.status,
                    created_at_from: parse_optional_rfc3339(req.created_at_from)?,
                    created_at_to: parse_optional_rfc3339(req.created_at_to)?,
                    updated_at_from: parse_optional_rfc3339(req.updated_at_from)?,
                    updated_at_to: parse_optional_rfc3339(req.updated_at_to)?,
                    sort_by: req.sort_by.map(|sort_by| match sort_by {
                        AppUserSortBy::CreatedAt => {
                            repo::table::app_users::AppUserSortBy::CreatedAt
                        }
                        AppUserSortBy::UpdatedAt => {
                            repo::table::app_users::AppUserSortBy::UpdatedAt
                        }
                    }),
                    sort_order: req.sort_order.map(|sort_order| match sort_order {
                        SortOrder::Asc => repo::table::app_users::SortOrder::Asc,
                        SortOrder::Desc => repo::table::app_users::SortOrder::Desc,
                    }),
                },
                req.pagination,
            )
            .await?;
        let user_ids = app_users
            .items
            .iter()
            .map(|user| user.user_id)
            .collect::<Vec<_>>();
        let user_roles = self.user_role_svc.list_by_user_ids(user_ids).await?;
        let role_ids = user_roles
            .iter()
            .map(|user_role| user_role.role_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let roles_by_id = self
            .role_svc
            .list_by_ids(role_ids)
            .await?
            .into_iter()
            .map(|role| (role.id, Self::map_role(role)))
            .collect::<HashMap<_, _>>();
        let mut roles_by_user_id = HashMap::<Uuid, Vec<RoleResponse>>::new();

        for user_role in user_roles {
            if let Some(role) = roles_by_id.get(&user_role.role_id) {
                roles_by_user_id
                    .entry(user_role.user_id)
                    .or_default()
                    .push(role.clone());
            }
        }

        Ok(app_users.map(|app_user| {
            let roles = roles_by_user_id
                .remove(&app_user.user_id)
                .unwrap_or_default();
            Self::map_app_user(app_user, roles)
        }))
    }

    pub async fn app_user_metrics(
        &self,
        current_admin_user_id: String,
    ) -> BizResult<AppUserMetricsResponse> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let metrics = self.app_user_svc.metrics().await?;
        let multi_role = self.user_role_svc.count_multi_role_users().await?;

        Ok(AppUserMetricsResponse {
            total: metrics.total,
            enabled: metrics.enabled,
            disabled: metrics.disabled,
            multi_role,
        })
    }

    pub async fn update_app_user(
        &self,
        current_admin_user_id: String,
        req: UpdateAppUserRequest,
    ) -> BizResult<AppUserResponse> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let user_id = parse_user_id(&req.user_id)?;
        self.ensure_app_user_exists(user_id).await?;
        let app_user = self
            .app_user_svc
            .update(UpdateAppUser {
                user_id,
                remark: req.remark,
                status: req.status,
            })
            .await?;
        let roles = self.list_roles_by_user_id(user_id).await?;

        Ok(Self::map_app_user(app_user, roles))
    }

    pub async fn delete_app_user(
        &self,
        current_admin_user_id: String,
        user_id: String,
    ) -> BizResult<()> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let user_id = parse_user_id(&user_id)?;
        self.ensure_app_user_exists(user_id).await?;
        let tx = self.db.begin().await?;
        let result: BizResult<()> = async {
            let user_role_svc = UserRoleService::new(tx.clone());
            let app_user_svc = AppUserService::new(tx.clone());

            user_role_svc.delete_by_user_id(user_id).await?;
            app_user_svc.delete_by_user_id(user_id).await?;

            Ok(())
        }
        .await;
        commit_or_rollback(tx, result).await?;

        Ok(())
    }

    pub async fn create_role(
        &self,
        current_admin_user_id: String,
        req: CreateRoleRequest,
    ) -> BizResult<RoleResponse> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLES)
            .await?;
        if req.code == DEFAULT_APP_ROLE_CODE {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                "role code 'free' is reserved".to_string(),
            ));
        }
        let role = self
            .role_svc
            .create(CreateRole {
                name: req.name,
                code: req.code,
            })
            .await?;
        Ok(Self::map_role(role))
    }

    pub async fn list_roles(&self, current_admin_user_id: String) -> BizResult<Vec<RoleResponse>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLES)
            .await?;
        Ok(self
            .role_svc
            .list_all()
            .await?
            .into_iter()
            .map(Self::map_role)
            .collect())
    }

    pub async fn delete_role(&self, current_admin_user_id: String, role_id: i64) -> BizResult<()> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLES)
            .await?;
        let role = self.ensure_role_exists(role_id).await?;
        if role.code == DEFAULT_APP_ROLE_CODE {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                "role code 'free' is reserved".to_string(),
            ));
        }
        let tx = self.db.begin().await?;
        let result: BizResult<()> = async {
            let role_permission_svc = RolePermissionService::new(tx.clone());
            let user_role_svc = UserRoleService::new(tx.clone());
            let role_svc = RoleService::new(tx.clone());

            role_permission_svc.delete_by_role_id(role_id).await?;
            user_role_svc.delete_by_role_id(role_id).await?;
            role_svc.delete_by_id(role_id).await?;

            Ok(())
        }
        .await;
        commit_or_rollback(tx, result).await?;

        Ok(())
    }

    pub async fn list_user_roles(
        &self,
        current_admin_user_id: String,
        user_id: String,
    ) -> BizResult<Vec<UserRoleOptionResponse>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let user_id = parse_user_id(&user_id)?;
        self.ensure_app_user_exists(user_id).await?;
        self.build_user_role_options(user_id).await
    }

    pub async fn update_user_roles(
        &self,
        current_admin_user_id: String,
        req: UpdateUserRolesRequest,
    ) -> BizResult<Vec<UserRoleOptionResponse>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_USERS)
            .await?;
        let user_id = parse_user_id(&req.user_id)?;
        self.ensure_app_user_exists(user_id).await?;
        let role_ids = req
            .role_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let roles = self.role_svc.list_by_ids(role_ids.clone()).await?;
        if roles.len() != role_ids.len() {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_NOT_FOUND,
                "one or more app roles not found".to_string(),
            ));
        }

        let tx = self.db.begin().await?;
        let result: BizResult<()> = async {
            let user_role_svc = UserRoleService::new(tx.clone());

            user_role_svc.delete_by_user_id(user_id).await?;
            for role_id in role_ids {
                user_role_svc
                    .create(CreateUserRole { user_id, role_id })
                    .await?;
            }

            Ok(())
        }
        .await;
        commit_or_rollback(tx, result).await?;

        self.build_user_role_options(user_id).await
    }

    pub async fn list_permissions(
        &self,
        current_admin_user_id: String,
    ) -> BizResult<Vec<PermissionTreeNode>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLE_PERMISSIONS)
            .await?;
        let permissions = self.permission_svc.list_all().await?;

        Ok(Self::build_plain_permission_tree(permissions, None))
    }

    pub async fn list_role_permissions(
        &self,
        current_admin_user_id: String,
        role_id: i64,
    ) -> BizResult<Vec<RolePermissionTreeNode>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLE_PERMISSIONS)
            .await?;
        self.ensure_role_exists(role_id).await?;
        self.build_role_permission_tree(role_id).await
    }

    pub async fn update_role_permissions(
        &self,
        current_admin_user_id: String,
        req: UpdateRolePermissionsRequest,
    ) -> BizResult<Vec<RolePermissionTreeNode>> {
        self.ensure_admin_permission(current_admin_user_id, PERM_APP_ROLE_PERMISSIONS)
            .await?;
        self.ensure_role_exists(req.role_id).await?;
        let permission_ids = req
            .permission_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let permissions = self
            .permission_svc
            .list_by_ids(permission_ids.clone())
            .await?;
        if permissions.len() != permission_ids.len() {
            return Err(BizError::new(
                admin_error::ADMIN_PERMISSION_NOT_FOUND,
                "one or more app permissions not found".to_string(),
            ));
        }

        let role_id = req.role_id;
        let tx = self.db.begin().await?;
        let result: BizResult<()> = async {
            let role_permission_svc = RolePermissionService::new(tx.clone());

            role_permission_svc.delete_by_role_id(role_id).await?;
            for permission_id in permission_ids {
                role_permission_svc
                    .create(CreateRolePermission {
                        role_id,
                        permission_id,
                    })
                    .await?;
            }

            Ok(())
        }
        .await;
        commit_or_rollback(tx, result).await?;

        self.build_role_permission_tree(role_id).await
    }

    pub async fn get_current_user_permissions(
        &self,
        user_id: String,
    ) -> BizResult<CurrentUserPermissionsResponse> {
        let access = self.ensure_app_user(&user_id).await?;
        let permission_codes = self
            .collect_permission_codes(access.role_ids.clone())
            .await?;

        Ok(CurrentUserPermissionsResponse {
            user_id,
            role_codes: access.role_codes,
            permission_codes,
        })
    }

    async fn ensure_admin_permission(
        &self,
        current_admin_user_id: String,
        permission_code: &str,
    ) -> BizResult<()> {
        self.admin_api
            .ensure_current_user_permission(current_admin_user_id, permission_code)
            .await
    }

    async fn ensure_app_user(&self, user_id: &str) -> BizResult<AppAccess> {
        let parsed_user_id = parse_user_id(user_id)?;
        let app_user = self.ensure_app_user_exists(parsed_user_id).await?;
        if app_user.status != AppUserStatus::Enabled {
            return Err(BizError::new(
                app_error::APP_USER_DISABLED,
                format!("app user is disabled: {user_id}"),
            ));
        }

        let user_roles = self.user_role_svc.list_by_user_id(parsed_user_id).await?;
        let role_ids: Vec<i64> = user_roles.into_iter().map(|item| item.role_id).collect();
        let role_codes = self
            .role_svc
            .list_by_ids(role_ids.clone())
            .await?
            .into_iter()
            .map(|role| role.code)
            .collect();

        Ok(AppAccess {
            role_ids,
            role_codes,
        })
    }

    async fn ensure_default_role_assigned(&self, user_id: Uuid) -> BizResult<()> {
        let default_role = self.ensure_default_role().await?;
        if self.user_role_svc.exists(user_id, default_role.id).await? {
            return Ok(());
        }

        match self
            .user_role_svc
            .create(CreateUserRole {
                user_id,
                role_id: default_role.id,
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                if self.user_role_svc.exists(user_id, default_role.id).await? {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn ensure_default_role(&self) -> BizResult<Role> {
        if let Some(role) = self.role_svc.get_by_code(DEFAULT_APP_ROLE_CODE).await? {
            return Ok(role);
        }

        match self
            .role_svc
            .create(CreateRole {
                name: DEFAULT_APP_ROLE_NAME.to_owned(),
                code: DEFAULT_APP_ROLE_CODE.to_owned(),
            })
            .await
        {
            Ok(role) => Ok(role),
            Err(err) => self
                .role_svc
                .get_by_code(DEFAULT_APP_ROLE_CODE)
                .await?
                .ok_or(err),
        }
    }

    async fn register_app_user_in_tx(
        &self,
        user_id: Uuid,
        req: RegisterAppUserRequest,
    ) -> BizResult<AppUser> {
        let existing_user = self.app_user_svc.get_by_user_id(user_id).await?;
        let app_user = if let Some(app_user) = existing_user {
            app_user
        } else {
            match self
                .app_user_svc
                .create(CreateAppUser {
                    user_id,
                    display_id: req.display_id,
                    display_name: req.display_name,
                    remark: req.remark,
                    status: AppUserStatus::Enabled,
                })
                .await
            {
                Ok(app_user) => app_user,
                Err(err) => self
                    .app_user_svc
                    .get_by_user_id(user_id)
                    .await?
                    .ok_or(err)?,
            }
        };

        if app_user.status == AppUserStatus::Enabled {
            self.ensure_default_role_assigned(user_id).await?;
        }

        Ok(app_user)
    }

    async fn ensure_app_user_exists(&self, user_id: Uuid) -> BizResult<AppUser> {
        self.app_user_svc
            .get_by_user_id(user_id)
            .await?
            .ok_or_else(|| {
                BizError::new(
                    app_error::APP_USER_NOT_FOUND,
                    format!("app user not found: {user_id}"),
                )
            })
    }

    async fn ensure_role_exists(&self, role_id: i64) -> BizResult<Role> {
        self.role_svc.get_by_id(role_id).await?.ok_or_else(|| {
            BizError::new(
                admin_error::ADMIN_ROLE_NOT_FOUND,
                format!("app role not found: {role_id}"),
            )
        })
    }

    async fn list_roles_by_user_id(&self, user_id: Uuid) -> BizResult<Vec<RoleResponse>> {
        let user_roles = self.user_role_svc.list_by_user_id(user_id).await?;
        let role_ids = user_roles.into_iter().map(|item| item.role_id).collect();

        Ok(self
            .role_svc
            .list_by_ids(role_ids)
            .await?
            .into_iter()
            .map(Self::map_role)
            .collect())
    }

    async fn build_user_role_options(
        &self,
        user_id: Uuid,
    ) -> BizResult<Vec<UserRoleOptionResponse>> {
        let checked_role_ids: HashSet<i64> = self
            .user_role_svc
            .list_by_user_id(user_id)
            .await?
            .into_iter()
            .map(|item| item.role_id)
            .collect();

        Ok(self
            .role_svc
            .list_all()
            .await?
            .into_iter()
            .map(|role| UserRoleOptionResponse {
                id: role.id,
                name: role.name,
                code: role.code,
                created_at: role.created_at,
                checked: checked_role_ids.contains(&role.id),
            })
            .collect())
    }

    async fn collect_permission_codes(&self, role_ids: Vec<i64>) -> BizResult<Vec<String>> {
        let role_permissions = self.role_permission_svc.list_by_role_ids(role_ids).await?;
        let permission_ids: Vec<i64> = role_permissions
            .into_iter()
            .map(|item| item.permission_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let codes: BTreeSet<String> = self
            .permission_svc
            .list_by_ids(permission_ids)
            .await?
            .into_iter()
            .map(|permission| permission.code)
            .collect();
        Ok(codes.into_iter().collect())
    }

    async fn build_role_permission_tree(
        &self,
        role_id: i64,
    ) -> BizResult<Vec<RolePermissionTreeNode>> {
        let checked_ids: HashSet<i64> = self
            .role_permission_svc
            .list_by_role_ids(vec![role_id])
            .await?
            .into_iter()
            .map(|item| item.permission_id)
            .collect();
        let permissions = self.permission_svc.list_all().await?;

        Ok(Self::build_permission_tree(permissions, &checked_ids, None))
    }

    fn build_plain_permission_tree(
        permissions: Vec<Permission>,
        parent_code: Option<&str>,
    ) -> Vec<PermissionTreeNode> {
        let mut nodes = permissions
            .iter()
            .filter(|permission| permission.parent_code.as_deref() == parent_code)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.sort.cmp(&right.sort).then(left.id.cmp(&right.id)));

        nodes
            .into_iter()
            .map(|permission| PermissionTreeNode {
                id: permission.id,
                name: permission.name.clone(),
                kind: permission.kind,
                created_at: permission.created_at,
                updated_at: permission.updated_at,
                children: Self::build_plain_permission_tree(
                    permissions.clone(),
                    Some(&permission.code),
                ),
            })
            .collect()
    }

    fn build_permission_tree(
        permissions: Vec<Permission>,
        checked_ids: &HashSet<i64>,
        parent_code: Option<&str>,
    ) -> Vec<RolePermissionTreeNode> {
        let mut nodes = permissions
            .iter()
            .filter(|permission| permission.parent_code.as_deref() == parent_code)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.sort.cmp(&right.sort).then(left.id.cmp(&right.id)));

        nodes
            .into_iter()
            .map(|permission| RolePermissionTreeNode {
                id: permission.id,
                name: permission.name.clone(),
                kind: permission.kind,
                created_at: permission.created_at,
                updated_at: permission.updated_at,
                checked: checked_ids.contains(&permission.id),
                children: Self::build_permission_tree(
                    permissions.clone(),
                    checked_ids,
                    Some(&permission.code),
                ),
            })
            .collect()
    }

    fn map_role(role: Role) -> RoleResponse {
        RoleResponse {
            id: role.id,
            name: role.name,
            code: role.code,
            created_at: role.created_at,
        }
    }

    fn map_app_user(app_user: AppUser, roles: Vec<RoleResponse>) -> AppUserResponse {
        AppUserResponse {
            user_id: app_user.user_id.to_string(),
            display_id: app_user.display_id,
            display_name: app_user.display_name,
            remark: app_user.remark,
            status: app_user.status,
            created_at: app_user.created_at,
            updated_at: app_user.updated_at,
            roles,
        }
    }
}

fn parse_user_id(user_id: &str) -> BizResult<Uuid> {
    Uuid::parse_str(user_id)
        .map_err(|err| BizError::new(BIZ_INTERNAL_ERROR, format!("invalid user_id uuid: {err}")))
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_optional_rfc3339(value: Option<String>) -> BizResult<Option<OffsetDateTime>> {
    normalize_optional_string(value)
        .map(|value| {
            OffsetDateTime::parse(&value, &Rfc3339).map_err(|err| {
                BizError::new(
                    BIZ_INTERNAL_ERROR,
                    format!("invalid RFC3339 datetime: {err}"),
                )
            })
        })
        .transpose()
}

async fn commit_or_rollback<T>(tx: DbContext, result: BizResult<T>) -> BizResult<T> {
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(err) => {
            tx.rollback().await?;
            Err(err)
        }
    }
}

struct AppAccess {
    role_ids: Vec<i64>,
    role_codes: Vec<String>,
}
