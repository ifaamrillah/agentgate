//! Logika satu entitas per file (rules §5); `_entities/` digenerate `bin/loco db entities`.

/// `ActiveModelBehavior` standar: id UUID v7 diisi saat insert, `updated_at` diperbarui saat
/// update (bila tabelnya punya kolom itu). Untuk model baru yang tidak butuh hook lain.
macro_rules! standard_behavior {
    ($am:ty) => {
        #[async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $am {
            async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                if insert && self.id.is_not_set() {
                    self.id = sea_orm::ActiveValue::Set(uuid::Uuid::now_v7());
                }
                if !insert && self.updated_at.is_unchanged() {
                    self.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                }
                Ok(self)
            }
        }
    };
    ($am:ty, append_only) => {
        #[async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $am {
            async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                if insert && self.id.is_not_set() {
                    self.id = sea_orm::ActiveValue::Set(uuid::Uuid::now_v7());
                }
                Ok(self)
            }
        }
    };
}

pub mod _entities;
pub mod artifact_versions;
pub mod audit_events;
pub mod components;
pub mod custom_fields;
pub mod git_connections;
pub mod graph_edges;
pub mod graph_files;
pub mod graph_links;
pub mod graph_repos;
pub mod graph_symbols;
pub mod identities;
pub mod invitations;
pub mod job_evidence;
pub mod job_logs;
pub mod job_permissions;
pub mod job_titles;
pub mod jobs;
pub mod knowledge_catalog;
pub mod knowledge_docs;
pub mod knowledge_proposals;
pub mod knowledge_runs;
pub mod knowledge_settings;
pub mod knowledge_sources;
pub mod knowledge_types;
pub mod knowledge_versions;
pub mod login_throttles;
pub mod notifications;
pub mod oauth_clients;
pub mod oauth_codes;
pub mod oauth_device_codes;
pub mod oauth_grants;
pub mod oauth_tokens;
pub mod office;
pub mod office_looks;
pub mod permission;
pub mod persona_skills;
pub mod personas;
pub mod project_access;
pub mod project_grants;
pub mod project_workflows;
pub mod projects;
pub mod pull_requests;
pub mod recovery_codes;
pub mod release_note_versions;
pub mod releases;
pub mod repos;
pub mod roles;
pub mod rule_backups;
pub mod rule_checks;
pub mod runner_prefs;
pub mod runners;
pub mod saved_filters;
pub mod skill_files;
pub mod skill_packs;
pub mod sprint_scope_events;
pub mod sprints;
pub mod team_members;
pub mod teams;
pub mod ticket_artifacts;
pub mod ticket_attachments;
pub mod ticket_code;
pub mod ticket_codes;
pub mod ticket_comments;
pub mod ticket_counters;
pub mod ticket_drafts;
pub mod ticket_estimates;
pub mod ticket_events;
pub mod ticket_links;
pub mod ticket_stage_people;
pub mod ticket_type_fields;
pub mod ticket_types;
pub mod ticket_watchers;
pub mod tickets;
pub mod token_savings;
pub mod user_tokens;
pub mod users;
pub mod workflow_versions;
pub mod workflows;
pub mod workspace_members;
pub mod workspaces;
