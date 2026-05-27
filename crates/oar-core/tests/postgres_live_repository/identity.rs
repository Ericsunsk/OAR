use super::harness::*;

#[test]
fn postgres_live_identity_repositories_upsert_lookup_and_tenant_conflict_guards() {
    run_live_postgres_test("identity_repo_upsert_lookup_conflict", |pool| async move {
        let tenant_repo = PostgresTenantRepository::new(pool.clone());
        let user_repo = PostgresWorkspaceUserRepository::new(pool.clone());
        let identity_repo = PostgresLarkIdentityRepository::new(pool.clone());

        let tenant_a = Tenant {
            id: TenantId("tenant_identity_a".to_string()),
            display_name: "Tenant A".to_string(),
            status: TenantStatus::Active,
        };
        let tenant_b = Tenant {
            id: TenantId("tenant_identity_b".to_string()),
            display_name: "Tenant B".to_string(),
            status: TenantStatus::Suspended,
        };

        let stored_tenant_a = tenant_repo.upsert(&tenant_a).await?;
        let stored_tenant_b = tenant_repo.upsert(&tenant_b).await?;
        assert_eq!(stored_tenant_a.status, TenantStatus::Active);
        assert_eq!(stored_tenant_b.status, TenantStatus::Suspended);

        let fetched_tenant = tenant_repo
            .get_by_id("tenant_identity_b")
            .await?
            .expect("tenant should exist");
        assert_eq!(fetched_tenant.display_name, "Tenant B");
        assert_eq!(fetched_tenant.status, TenantStatus::Suspended);

        let user_a = WorkspaceUser {
            id: WorkspaceUserId("user_identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            display_name: "User Shared A".to_string(),
            status: WorkspaceUserStatus::Active,
        };
        let stored_user_a = user_repo.upsert(&user_a).await?;
        assert_eq!(stored_user_a.tenant_id, "tenant_identity_a");
        assert_eq!(stored_user_a.status, WorkspaceUserStatus::Active);

        let fetched_user_a = user_repo
            .get_by_id("tenant_identity_a", "user_identity_shared")
            .await?
            .expect("user should exist for tenant A");
        assert_eq!(fetched_user_a.display_name, "User Shared A");
        assert_eq!(
            user_repo
                .get_by_id("tenant_identity_b", "user_identity_shared")
                .await?,
            None
        );

        let conflicting_user = WorkspaceUser {
            id: WorkspaceUserId("user_identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_b".to_string()),
            display_name: "User Shared B".to_string(),
            status: WorkspaceUserStatus::Disabled,
        };
        match user_repo.upsert(&conflicting_user).await {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_identity_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch for workspace_users, got {other:?}"),
        }

        let identity_a = LarkIdentity {
            id: LarkIdentityId("identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            actor_kind: ActorKind::User,
            actor_external_id: "ext-shared-a".to_string(),
            display_name: Some("Identity Shared A".to_string()),
        };
        let stored_identity_a = identity_repo.upsert(&identity_a).await?;
        assert_eq!(stored_identity_a.tenant_id, "tenant_identity_a");
        assert_eq!(stored_identity_a.actor_kind, ActorKind::User);

        let fetched_identity_a = identity_repo
            .get_by_id("tenant_identity_a", "identity_shared")
            .await?
            .expect("identity should exist for tenant A");
        assert_eq!(fetched_identity_a.actor_external_id, "ext-shared-a");

        let fetched_by_external = identity_repo
            .get_by_actor_external_id("tenant_identity_a", ActorKind::User, "ext-shared-a")
            .await?
            .expect("identity should be discoverable by external actor id");
        assert_eq!(fetched_by_external.id, "identity_shared");
        assert_eq!(
            identity_repo
                .get_by_actor_external_id("tenant_identity_b", ActorKind::User, "ext-shared-a")
                .await?,
            None
        );

        let duplicate_external_binding = LarkIdentity {
            id: LarkIdentityId("identity_duplicate_external".to_string()),
            tenant_id: TenantId("tenant_identity_a".to_string()),
            actor_kind: ActorKind::User,
            actor_external_id: "ext-shared-a".to_string(),
            display_name: Some("Identity Duplicate External".to_string()),
        };
        match identity_repo.upsert(&duplicate_external_binding).await {
            Err(PostgresRepositoryError::LarkIdentityActorExternalBindingConflict {
                tenant_id,
                actor_kind,
                actor_external_id,
            }) => {
                assert_eq!(tenant_id, "tenant_identity_a");
                assert_eq!(actor_kind, ActorKind::User);
                assert_eq!(actor_external_id, "ext-shared-a");
            }
            other => panic!(
                "expected typed actor external binding conflict for lark_identities, got {other:?}"
            ),
        }

        let conflicting_identity = LarkIdentity {
            id: LarkIdentityId("identity_shared".to_string()),
            tenant_id: TenantId("tenant_identity_b".to_string()),
            actor_kind: ActorKind::Bot,
            actor_external_id: "ext-shared-b".to_string(),
            display_name: Some("Identity Shared B".to_string()),
        };
        match identity_repo.upsert(&conflicting_identity).await {
            Err(PostgresRepositoryError::TenantMismatch {
                field,
                expected,
                actual,
            }) => {
                assert_eq!(field, "tenant_id");
                assert_eq!(expected, "tenant_identity_b");
                assert_eq!(actual, "<redacted>");
            }
            other => panic!("expected tenant mismatch for lark_identities, got {other:?}"),
        }

        Ok(())
    });
}
