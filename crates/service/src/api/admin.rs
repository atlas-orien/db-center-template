use std::collections::{BTreeSet, HashMap, HashSet};

use db_core::{
    DbContext,
    error::{BIZ_INTERNAL_ERROR, BizError, BizResult},
};
use error_code::admin as admin_error;
use repo::table::{
    admin_menus::{CreateMenu, Menu, MenuService},
    admin_permissions::{Permission, PermissionService},
    admin_role_permissions::{CreateRolePermission, RolePermissionService},
    admin_roles::{CreateRole, Role, RoleService},
    admin_user_roles::{CreateUserRole, UserRoleService},
    admin_users::{AdminUser, AdminUserService, AdminUserStatus, CreateAdminUser, UpdateAdminUser},
};
use uuid::Uuid;

use crate::dto::admin::{
    AdminUserResponse, CreateAdminUserRequest, CreateMenuRequest, CreateRoleRequest,
    CurrentUserPermissionsResponse, MenuResponse, MenuTreeNode, PermissionTreeNode,
    RolePermissionTreeNode, RoleResponse, UpdateAdminUserRequest, UpdateRolePermissionsRequest,
    UpdateUserRolesRequest, UserRoleOptionResponse,
};

const ADMIN_ROLE_CODE: &str = "admin";
const ROOT_ROLE_CODE: &str = "root";
const SUPPORT_ROLE_CODE: &str = "support";
const PERM_ADMIN_USERS: &str = "accounts:admin_users";
const PERM_ROLES: &str = "access_control:roles";
const PERM_ROLE_PERMISSIONS: &str = "access_control:role_permissions";

pub struct AdminApi {
    db: DbContext,
    admin_user_svc: AdminUserService,
    role_svc: RoleService,
    permission_svc: PermissionService,
    user_role_svc: UserRoleService,
    role_permission_svc: RolePermissionService,
    menu_svc: MenuService,
}

impl AdminApi {
    pub fn new(db: DbContext) -> Self {
        Self {
            db: db.clone(),
            admin_user_svc: AdminUserService::new(db.clone()),
            role_svc: RoleService::new(db.clone()),
            permission_svc: PermissionService::new(db.clone()),
            user_role_svc: UserRoleService::new(db.clone()),
            role_permission_svc: RolePermissionService::new(db.clone()),
            menu_svc: MenuService::new(db),
        }
    }

    pub async fn ensure_current_user_permission(
        &self,
        current_user_id: String,
        permission_code: &str,
    ) -> BizResult<()> {
        self.ensure_permission(&current_user_id, permission_code)
            .await?;
        Ok(())
    }

    pub async fn create_admin_user(
        &self,
        current_user_id: String,
        req: CreateAdminUserRequest,
    ) -> BizResult<AdminUserResponse> {
        self.ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let user_id = parse_user_id(&req.user_id)?;
        let admin_user = self
            .admin_user_svc
            .create(CreateAdminUser {
                user_id,
                display_id: req.display_id,
                display_name: req.display_name,
                remark: req.remark,
                status: AdminUserStatus::Enabled,
            })
            .await?;

        Ok(Self::map_admin_user(admin_user, Vec::new()))
    }

    pub async fn list_admin_users(
        &self,
        current_user_id: String,
    ) -> BizResult<Vec<AdminUserResponse>> {
        self.ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let admin_users = self.admin_user_svc.list_all().await?;
        let user_ids = admin_users
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

        Ok(admin_users
            .into_iter()
            .filter_map(|admin_user| {
                let roles = roles_by_user_id
                    .remove(&admin_user.user_id)
                    .unwrap_or_default();
                if roles.iter().any(|role| role.code == ROOT_ROLE_CODE) {
                    return None;
                }

                Some(Self::map_admin_user(admin_user, roles))
            })
            .collect())
    }

    pub async fn update_admin_user(
        &self,
        current_user_id: String,
        req: UpdateAdminUserRequest,
    ) -> BizResult<AdminUserResponse> {
        let access = self
            .ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let user_id = parse_user_id(&req.user_id)?;
        self.ensure_admin_user_change_allowed(&access, user_id, "only root can update root user")
            .await?;
        self.ensure_admin_user_exists(user_id).await?;
        let admin_user = self
            .admin_user_svc
            .update(UpdateAdminUser {
                user_id,
                remark: req.remark,
                status: req.status,
            })
            .await?;
        let roles = self.list_roles_by_user_id(user_id).await?;

        Ok(Self::map_admin_user(admin_user, roles))
    }

    pub async fn delete_admin_user(
        &self,
        current_user_id: String,
        user_id: String,
    ) -> BizResult<()> {
        let access = self
            .ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let user_id = parse_user_id(&user_id)?;
        self.ensure_admin_user_change_allowed(&access, user_id, "only root can delete root user")
            .await?;
        self.ensure_admin_user_exists(user_id).await?;
        let tx = self.db.begin().await?;
        let result: BizResult<()> = async {
            let user_role_svc = UserRoleService::new(tx.clone());
            let admin_user_svc = AdminUserService::new(tx.clone());

            user_role_svc.delete_by_user_id(user_id).await?;
            admin_user_svc.delete_by_user_id(user_id).await?;

            Ok(())
        }
        .await;
        commit_or_rollback(tx, result).await?;

        Ok(())
    }

    pub async fn create_role(
        &self,
        current_user_id: String,
        req: CreateRoleRequest,
    ) -> BizResult<RoleResponse> {
        self.ensure_permission(&current_user_id, PERM_ROLES).await?;
        if is_reserved_role_code(&req.code) {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                format!("role code '{}' is reserved", req.code),
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

    pub async fn list_roles(&self, current_user_id: String) -> BizResult<Vec<RoleResponse>> {
        self.ensure_permission(&current_user_id, PERM_ROLES).await?;
        Ok(self
            .role_svc
            .list_all()
            .await?
            .into_iter()
            .filter(|role| role.code != ROOT_ROLE_CODE)
            .map(Self::map_role)
            .collect())
    }

    pub async fn delete_role(&self, current_user_id: String, role_id: i64) -> BizResult<()> {
        self.ensure_permission(&current_user_id, PERM_ROLES).await?;
        let role = self.ensure_role_exists(role_id).await?;
        if is_reserved_role_code(&role.code) {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                format!("role code '{}' is reserved", role.code),
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

    pub async fn create_menu(
        &self,
        current_user_id: String,
        req: CreateMenuRequest,
    ) -> BizResult<MenuResponse> {
        self.ensure_permission(&current_user_id, PERM_ROLE_PERMISSIONS)
            .await?;
        let menu = self
            .menu_svc
            .create(CreateMenu {
                name: req.name,
                parent_id: req.parent_id,
                permission_code: None,
            })
            .await?;
        Ok(Self::map_menu(menu))
    }

    pub async fn list_menus(&self, current_user_id: String) -> BizResult<Vec<MenuResponse>> {
        self.ensure_permission(&current_user_id, PERM_ROLE_PERMISSIONS)
            .await?;
        Ok(self
            .menu_svc
            .list_all()
            .await?
            .into_iter()
            .map(Self::map_menu)
            .collect())
    }

    pub async fn list_user_roles(
        &self,
        current_user_id: String,
        user_id: String,
    ) -> BizResult<Vec<UserRoleOptionResponse>> {
        let access = self
            .ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let user_id = parse_user_id(&user_id)?;
        self.ensure_admin_user_exists(user_id).await?;
        self.build_user_role_options(user_id, &access).await
    }

    pub async fn update_user_roles(
        &self,
        current_user_id: String,
        req: UpdateUserRolesRequest,
    ) -> BizResult<Vec<UserRoleOptionResponse>> {
        let access = self
            .ensure_permission(&current_user_id, PERM_ADMIN_USERS)
            .await?;
        let user_id = parse_user_id(&req.user_id)?;
        self.ensure_admin_user_exists(user_id).await?;
        self.ensure_admin_user_change_allowed(
            &access,
            user_id,
            "only root can update root user roles",
        )
        .await?;

        let role_ids = req
            .role_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for role_id in &role_ids {
            self.ensure_role_assignment_allowed(&access, *role_id)
                .await?;
        }

        let roles = self.role_svc.list_by_ids(role_ids.clone()).await?;
        if roles.len() != role_ids.len() {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_NOT_FOUND,
                "one or more roles not found".to_string(),
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

        self.build_user_role_options(user_id, &access).await
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
        access: &AdminAccess,
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
            .filter(|role| access.is_root() || role.code != ROOT_ROLE_CODE)
            .map(|role| UserRoleOptionResponse {
                id: role.id,
                name: role.name,
                code: role.code,
                created_at: role.created_at,
                checked: checked_role_ids.contains(&role.id),
            })
            .collect())
    }

    pub async fn list_permissions(
        &self,
        current_user_id: String,
    ) -> BizResult<Vec<PermissionTreeNode>> {
        self.ensure_permission(&current_user_id, PERM_ROLE_PERMISSIONS)
            .await?;
        let permissions = self.permission_svc.list_all().await?;

        Ok(Self::build_plain_permission_tree(permissions, None))
    }

    pub async fn list_role_permissions(
        &self,
        current_user_id: String,
        role_id: i64,
    ) -> BizResult<Vec<RolePermissionTreeNode>> {
        self.ensure_permission(&current_user_id, PERM_ROLE_PERMISSIONS)
            .await?;
        self.ensure_role_exists(role_id).await?;
        self.build_role_permission_tree(role_id).await
    }

    pub async fn update_role_permissions(
        &self,
        current_user_id: String,
        req: UpdateRolePermissionsRequest,
    ) -> BizResult<Vec<RolePermissionTreeNode>> {
        let access = self
            .ensure_permission(&current_user_id, PERM_ROLE_PERMISSIONS)
            .await?;
        self.ensure_role_exists(req.role_id).await?;
        self.ensure_role_permission_change_allowed(&access, req.role_id)
            .await?;

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
                "one or more permissions not found".to_string(),
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
        let access = self.ensure_admin_user(&user_id).await?;
        let permission_codes = if access.is_root() {
            self.permission_svc
                .list_all()
                .await?
                .into_iter()
                .map(|permission| permission.code)
                .collect()
        } else {
            self.collect_permission_codes(access.role_ids.clone())
                .await?
        };

        Ok(CurrentUserPermissionsResponse {
            user_id,
            role_codes: access.role_codes,
            permission_codes,
        })
    }

    pub async fn get_current_user_menus(&self, user_id: String) -> BizResult<Vec<MenuTreeNode>> {
        let access = self.ensure_admin_user(&user_id).await?;
        let all_menus = self.menu_svc.list_all().await?;
        if access.is_root() {
            return Ok(Self::build_menu_tree(all_menus));
        }

        let permission_codes: HashSet<String> = self
            .collect_permission_codes(access.role_ids)
            .await?
            .into_iter()
            .collect();
        let menu_map: HashMap<i64, Menu> = all_menus
            .iter()
            .cloned()
            .map(|menu| (menu.id, menu))
            .collect();

        let direct_ids: Vec<i64> = all_menus
            .iter()
            .filter(|menu| {
                menu.permission_code
                    .as_ref()
                    .is_some_and(|code| permission_codes.contains(code))
            })
            .map(|menu| menu.id)
            .collect();

        let mut include_ids: HashSet<i64> = HashSet::new();
        for menu_id in direct_ids {
            self.collect_ancestors(menu_id, &menu_map, &mut include_ids);
        }

        let included_menus: Vec<Menu> = all_menus
            .into_iter()
            .filter(|menu| include_ids.contains(&menu.id))
            .collect();

        Ok(Self::build_menu_tree(included_menus))
    }

    async fn ensure_admin_user(&self, user_id: &str) -> BizResult<AdminAccess> {
        let parsed_user_id = parse_user_id(user_id)?;
        let admin_user = self.ensure_admin_user_exists(parsed_user_id).await?;

        if admin_user.status != AdminUserStatus::Enabled {
            return Err(BizError::new(
                admin_error::ADMIN_USER_DISABLED,
                format!("admin user is disabled: {user_id}"),
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

        Ok(AdminAccess {
            role_ids,
            role_codes,
        })
    }

    async fn ensure_admin_user_exists(&self, user_id: Uuid) -> BizResult<AdminUser> {
        self.admin_user_svc
            .get_by_user_id(user_id)
            .await?
            .ok_or_else(|| {
                BizError::new(
                    admin_error::ADMIN_USER_NOT_FOUND,
                    format!("admin user not found: {user_id}"),
                )
            })
    }

    async fn ensure_role_exists(&self, role_id: i64) -> BizResult<Role> {
        self.role_svc.get_by_id(role_id).await?.ok_or_else(|| {
            BizError::new(
                admin_error::ADMIN_ROLE_NOT_FOUND,
                format!("role not found: {role_id}"),
            )
        })
    }

    async fn ensure_permission(
        &self,
        user_id: &str,
        permission_code: &str,
    ) -> BizResult<AdminAccess> {
        let access = self.ensure_admin_user(user_id).await?;
        if access.is_root() {
            return Ok(access);
        }

        let permission_codes = self
            .collect_permission_codes(access.role_ids.clone())
            .await?;
        if permission_codes.iter().any(|code| code == permission_code) {
            return Ok(access);
        }

        Err(BizError::new(
            admin_error::ADMIN_PERMISSION_DENIED,
            format!("permission denied: {permission_code}"),
        ))
    }

    async fn ensure_role_assignment_allowed(
        &self,
        access: &AdminAccess,
        role_id: i64,
    ) -> BizResult<()> {
        if access.is_root() {
            return Ok(());
        }

        if self.is_root_role_id(role_id).await? {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                "only root can assign root role".to_string(),
            ));
        }

        Ok(())
    }

    async fn ensure_role_permission_change_allowed(
        &self,
        access: &AdminAccess,
        role_id: i64,
    ) -> BizResult<()> {
        if access.is_root() {
            return Ok(());
        }

        if self.is_root_role_id(role_id).await? {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                "only root can modify root role permissions".to_string(),
            ));
        }

        Ok(())
    }

    async fn ensure_admin_user_change_allowed(
        &self,
        access: &AdminAccess,
        user_id: Uuid,
        message: &str,
    ) -> BizResult<()> {
        if access.is_root() {
            return Ok(());
        }

        let roles = self.list_roles_by_user_id(user_id).await?;
        if roles.iter().any(|role| role.code == ROOT_ROLE_CODE) {
            return Err(BizError::new(
                admin_error::ADMIN_ROLE_RESERVED,
                message.to_string(),
            ));
        }

        Ok(())
    }

    async fn is_root_role_id(&self, role_id: i64) -> BizResult<bool> {
        Ok(self
            .role_svc
            .list_by_ids(vec![role_id])
            .await?
            .into_iter()
            .any(|role| role.code == ROOT_ROLE_CODE))
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

    fn collect_ancestors(
        &self,
        menu_id: i64,
        menu_map: &HashMap<i64, Menu>,
        include_ids: &mut HashSet<i64>,
    ) {
        let mut current = Some(menu_id);
        let mut visited = HashSet::new();

        while let Some(id) = current {
            if !visited.insert(id) {
                break;
            }
            include_ids.insert(id);
            current = menu_map.get(&id).and_then(|menu| menu.parent_id);
        }
    }

    fn build_menu_tree(menus: Vec<Menu>) -> Vec<MenuTreeNode> {
        let nodes: HashMap<i64, MenuTreeNode> = menus
            .into_iter()
            .map(|menu| {
                (
                    menu.id,
                    MenuTreeNode {
                        id: menu.id,
                        name: menu.name,
                        parent_id: menu.parent_id,
                        created_at: menu.created_at,
                        updated_at: menu.updated_at,
                        children: Vec::new(),
                    },
                )
            })
            .collect();

        let mut parent_children: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut roots = Vec::new();

        for node in nodes.values() {
            if let Some(parent_id) = node.parent_id {
                parent_children.entry(parent_id).or_default().push(node.id);
            } else {
                roots.push(node.id);
            }
        }

        roots.sort_unstable();
        for children in parent_children.values_mut() {
            children.sort_unstable();
        }

        fn build_node(
            id: i64,
            nodes: &HashMap<i64, MenuTreeNode>,
            parent_children: &HashMap<i64, Vec<i64>>,
        ) -> MenuTreeNode {
            let mut node = nodes.get(&id).cloned().expect("menu node must exist");
            if let Some(child_ids) = parent_children.get(&id) {
                node.children = child_ids
                    .iter()
                    .map(|child_id| build_node(*child_id, nodes, parent_children))
                    .collect();
            }
            node
        }

        roots
            .into_iter()
            .map(|root_id| build_node(root_id, &nodes, &parent_children))
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

    fn map_admin_user(admin_user: AdminUser, roles: Vec<RoleResponse>) -> AdminUserResponse {
        AdminUserResponse {
            user_id: admin_user.user_id.to_string(),
            display_id: admin_user.display_id,
            display_name: admin_user.display_name,
            remark: admin_user.remark,
            status: admin_user.status,
            created_at: admin_user.created_at,
            updated_at: admin_user.updated_at,
            roles,
        }
    }

    fn map_menu(menu: Menu) -> MenuResponse {
        MenuResponse {
            id: menu.id,
            name: menu.name,
            parent_id: menu.parent_id,
            created_at: menu.created_at,
            updated_at: menu.updated_at,
        }
    }
}

fn parse_user_id(user_id: &str) -> BizResult<Uuid> {
    Uuid::parse_str(user_id)
        .map_err(|err| BizError::new(BIZ_INTERNAL_ERROR, format!("invalid user_id uuid: {err}")))
}

struct AdminAccess {
    role_ids: Vec<i64>,
    role_codes: Vec<String>,
}

impl AdminAccess {
    fn is_root(&self) -> bool {
        self.role_codes.iter().any(|code| code == ROOT_ROLE_CODE)
    }
}

fn is_reserved_role_code(code: &str) -> bool {
    matches!(code, ROOT_ROLE_CODE | ADMIN_ROLE_CODE | SUPPORT_ROLE_CODE)
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
