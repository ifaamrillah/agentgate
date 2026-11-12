//! M3 Workflow & gate: workflow per workspace (versi + draf), workflow per tipe tiket (default
//! workspace + override per project), stage tiket, penugasan per stage, artifact + versi, dan
//! kind riwayat/notifikasi/audit baru.
//!
//! - `workflow_versions.definition` = `agentgate_core::workflow::Definition` (serde JSON).
//!   `workflows.version` = versi aktif; `workflows.draft` = draf builder yang belum disimpan.
//! - Tiket menyimpan `stage_key` (kunci stage, stabil antar versi) dan `stage_version` (versi
//!   yang aturannya berlaku untuk stage sekarang). `tickets.status` tetap = kategori stage.
//! - Backfill: setiap workspace yang sudah ada mendapat workflow Kanban (default semua tipe)
//!   dan Subtask ringkas (tipe Subtask); tiket lama masuk Kanban di stage = status-nya
//!   (kunci stage Kanban sama dengan nilai status). JSON di bawah harus sama dengan template
//!   `core/templates/{kanban,subtask}.yaml` (dijaga tes `workflow_backfill_matches_templates`).
//!
//! Kode rilis M2 tidak menulis kolom tiket baru, jadi kolom NOT NULL baru membuat kode M2
//! gagal membuat tiket: belum ada deploy production, jadi tidak dipecah expand/contract.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Definisi Kanban untuk backfill (sama dengan template `kanban`).
pub const KANBAN_JSON: &str = r#"{"stages":[{"key":"backlog","name":"Backlog","category":"backlog"},{"key":"todo","name":"To do","category":"todo"},{"key":"in_progress","name":"Dikerjakan","category":"in_progress"},{"key":"done","name":"Selesai","category":"done"}]}"#;
pub const KANBAN_DESCRIPTION: &str =
    "Alur sederhana tanpa agent. Tiket dipindah manusia lewat board.";
/// Definisi Subtask ringkas untuk backfill (sama dengan template `subtask`).
pub const SUBTASK_JSON: &str = r#"{"stages":[{"key":"todo","name":"To do","category":"todo"},{"key":"dev","name":"Dev","category":"in_progress"},{"key":"review","name":"Review","category":"in_progress","exit":"approval"},{"key":"done","name":"Selesai","category":"done"}]}"#;
pub const SUBTASK_DESCRIPTION: &str =
    "Workflow ringkas untuk subtask, Dev lalu Review lalu Selesai.";

const AUDIT_ACTIONS_M2: &str =
    "'workspace.created', 'workspace.updated', 'workspace.require_2fa_changed',
        'workspace.deletion_scheduled', 'workspace.deletion_cancelled',
        'member.invited', 'member.joined', 'member.role_changed', 'member.removed',
        'member.left', 'member.job_title_changed', 'member.invitation_revoked',
        'role.created', 'role.updated', 'role.deleted',
        'job_title.created', 'job_title.updated', 'job_title.deleted',
        'team.created', 'team.updated', 'team.deleted', 'team.member_added',
        'team.member_removed',
        'project.created', 'project.updated', 'project.archived',
        'project.access_granted', 'project.access_changed', 'project.access_revoked',
        'repo.added', 'repo.updated', 'repo.removed',
        'ticket.trashed', 'ticket.restored', 'ticket.purged',
        'ticket_type.created', 'ticket_type.updated', 'ticket_type.deleted',
        'custom_field.created', 'custom_field.updated', 'custom_field.deleted',
        'ticket_code.updated'";
const AUDIT_SUBJECTS_M2: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field'";
const EVENT_KINDS_M2: &str = "'created', 'updated', 'status_changed', 'linked', 'unlinked',
        'attachment_added', 'attachment_removed', 'trashed', 'restored'";
const NOTIFICATION_KINDS_M2: &str = "'mentioned', 'assigned', 'commented'";

fn up() -> Vec<String> {
    let mut sql: Vec<String> = vec![
        "SET lock_timeout = '5s'".into(),
        // --- audit, riwayat, notifikasi ---
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M2},
            'workflow.created', 'workflow.updated', 'workflow.deleted', 'workflow.migrated',
            'project.workflows_changed', 'ticket.stage_jumped', 'ticket.transition_forced'))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M2}, 'workflow'))"
        ),
        "ALTER TABLE ticket_events DROP CONSTRAINT ticket_events_kind_check".into(),
        format!(
            "ALTER TABLE ticket_events ADD CONSTRAINT ticket_events_kind_check CHECK (kind IN (
            {EVENT_KINDS_M2},
            'stage_changed', 'review_requested', 'approved', 'rejected', 'decision_needed',
            'decided', 'stage_jumped', 'transition_forced', 'workflow_migrated',
            'artifact_saved', 'people_changed'))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M2},
            'review_requested', 'approved', 'rejected', 'decision_needed'))"
        ),
        // --- workflow ---
        // key: slug unik per workspace (nama berkas ekspor YAML). template_key: asal template.
        "CREATE TABLE workflows (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            key text NOT NULL,
            name text NOT NULL,
            description text NOT NULL DEFAULT '',
            template_key text,
            version integer NOT NULL DEFAULT 1,
            draft jsonb,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            UNIQUE (workspace_id, key)
        )"
        .into(),
        "CREATE UNIQUE INDEX workflows_workspace_id_name_lower_key \
         ON workflows (workspace_id, lower(name))"
            .into(),
        // Snapshot definisi per versi: riwayat versi dan aturan stage yang berlaku untuk tiket
        // yang belum pindah stage sejak workflow diubah.
        "CREATE TABLE workflow_versions (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            workflow_id uuid NOT NULL,
            version integer NOT NULL,
            definition jsonb NOT NULL,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workflow_id, version),
            FOREIGN KEY (workflow_id, workspace_id) REFERENCES workflows (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX workflow_versions_workspace_id_idx ON workflow_versions (workspace_id)"
            .into(),
        // Workflow default workspace per tipe. Tanpa ON DELETE: workflow yang dipakai tidak bisa
        // dihapus.
        "ALTER TABLE ticket_types ADD COLUMN workflow_id uuid".into(),
        "ALTER TABLE ticket_types ADD CONSTRAINT ticket_types_workflow_fkey \
         FOREIGN KEY (workflow_id, workspace_id) REFERENCES workflows (id, workspace_id)"
            .into(),
        "CREATE INDEX ticket_types_workflow_id_idx ON ticket_types (workflow_id)".into(),
        // Override per project per tipe (hapus project/tipe menghapus override-nya).
        "CREATE TABLE project_workflows (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            type_id uuid NOT NULL,
            workflow_id uuid NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (project_id, type_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (type_id, workspace_id) REFERENCES ticket_types (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (workflow_id, workspace_id) REFERENCES workflows (id, workspace_id)
        )"
        .into(),
        "CREATE INDEX project_workflows_workflow_id_idx ON project_workflows (workflow_id)".into(),
        "CREATE INDEX project_workflows_type_id_idx ON project_workflows (type_id)".into(),
        "CREATE INDEX project_workflows_workspace_id_idx ON project_workflows (workspace_id)"
            .into(),
        // Aturan transisi per project (prototype "Workflow project").
        "ALTER TABLE projects ADD COLUMN allow_force_transition boolean NOT NULL DEFAULT true"
            .into(),
        // --- stage tiket ---
        // stage_state: active = dikerjakan, review = menunggu approve, decision = Butuh keputusan.
        // fail_count/fail_stage_key: putaran gagal berturut-turut di stage fail_stage_key.
        "ALTER TABLE tickets
            ADD COLUMN workflow_id uuid,
            ADD COLUMN stage_key text,
            ADD COLUMN stage_version integer,
            ADD COLUMN stage_state text NOT NULL DEFAULT 'active'
                CHECK (stage_state IN ('active', 'review', 'decision')),
            ADD COLUMN fail_count integer NOT NULL DEFAULT 0,
            ADD COLUMN fail_stage_key text,
            ADD COLUMN stage_entered_at timestamptz NOT NULL DEFAULT now(),
            ADD COLUMN cancelled_at timestamptz"
            .into(),
        "ALTER TABLE tickets ADD CONSTRAINT tickets_workflow_fkey \
         FOREIGN KEY (workflow_id, workspace_id) REFERENCES workflows (id, workspace_id)"
            .into(),
        "CREATE INDEX tickets_workflow_id_stage_key_idx ON tickets (workflow_id, stage_key) \
         WHERE deleted_at IS NULL"
            .into(),
        // Kotak masuk "Butuh keputusan": tiket yang menunggu review/keputusan (sisi jarang).
        "CREATE INDEX tickets_stage_state_idx ON tickets (workspace_id, stage_state) \
         WHERE stage_state <> 'active' AND deleted_at IS NULL"
            .into(),
    ];
    sql.extend(backfill());
    sql.extend([
        "ALTER TABLE ticket_types ALTER COLUMN workflow_id SET NOT NULL".to_string(),
        "ALTER TABLE tickets ALTER COLUMN workflow_id SET NOT NULL,
            ALTER COLUMN stage_key SET NOT NULL,
            ALTER COLUMN stage_version SET NOT NULL"
            .into(),
        // Penugasan per stage tiket. Anggota yang keluar dari workspace ikut terhapus.
        "CREATE TABLE ticket_stage_people (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            ticket_id uuid NOT NULL,
            stage_key text NOT NULL,
            role text NOT NULL CHECK (role IN ('executor', 'approver', 'watcher')),
            user_id uuid NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (ticket_id, stage_key, role, user_id),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX ticket_stage_people_user_idx ON ticket_stage_people (workspace_id, user_id)"
            .into(),
        // Artifact tiket: nama unik per tiket; versi v1, v2, … (body Markdown).
        "CREATE TABLE ticket_artifacts (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            ticket_id uuid NOT NULL,
            stage_key text NOT NULL,
            name text NOT NULL,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE UNIQUE INDEX ticket_artifacts_ticket_id_name_lower_key \
         ON ticket_artifacts (ticket_id, lower(name))"
            .into(),
        "CREATE INDEX ticket_artifacts_workspace_id_idx ON ticket_artifacts (workspace_id)".into(),
        // status: draft (belum diajukan), pending (menunggu review), approved, rejected.
        "CREATE TABLE artifact_versions (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            artifact_id uuid NOT NULL,
            number integer NOT NULL,
            body text NOT NULL,
            status text NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft', 'pending', 'approved', 'rejected')),
            note text NOT NULL DEFAULT '',
            author_id uuid REFERENCES users (id) ON DELETE SET NULL,
            decided_by uuid REFERENCES users (id) ON DELETE SET NULL,
            decided_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (artifact_id, number),
            FOREIGN KEY (artifact_id, workspace_id) REFERENCES ticket_artifacts (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX artifact_versions_workspace_id_idx ON artifact_versions (workspace_id)"
            .into(),
    ]);
    sql
}

/// Workflow bawaan untuk workspace yang sudah ada, tipe → workflow, tiket → Kanban.
fn backfill() -> Vec<String> {
    let insert = |key: &str, name: &str, description: &str, json: &str| {
        format!(
            "WITH w AS (
                INSERT INTO workflows (workspace_id, key, name, description, template_key)
                SELECT id, '{key}', '{name}', '{description}', '{key}' FROM workspaces
                RETURNING id, workspace_id
            )
            INSERT INTO workflow_versions (workspace_id, workflow_id, version, definition)
            SELECT workspace_id, id, 1, '{json}'::jsonb FROM w"
        )
    };
    vec![
        insert("kanban", "Kanban", KANBAN_DESCRIPTION, KANBAN_JSON),
        insert(
            "subtask",
            "Subtask ringkas",
            SUBTASK_DESCRIPTION,
            SUBTASK_JSON,
        ),
        "UPDATE ticket_types t SET workflow_id = w.id FROM workflows w
         WHERE w.workspace_id = t.workspace_id
           AND w.key = CASE WHEN t.key = 'subtask' THEN 'subtask' ELSE 'kanban' END"
            .into(),
        "UPDATE tickets t SET workflow_id = w.id, stage_key = t.status, stage_version = 1,
            stage_entered_at = t.updated_at
         FROM workflows w WHERE w.workspace_id = t.workspace_id AND w.key = 'kanban'"
            .into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS artifact_versions".into(),
        "DROP TABLE IF EXISTS ticket_artifacts".into(),
        "DROP TABLE IF EXISTS ticket_stage_people".into(),
        "ALTER TABLE tickets DROP CONSTRAINT IF EXISTS tickets_workflow_fkey,
            DROP COLUMN IF EXISTS workflow_id, DROP COLUMN IF EXISTS stage_key,
            DROP COLUMN IF EXISTS stage_version, DROP COLUMN IF EXISTS stage_state,
            DROP COLUMN IF EXISTS fail_count, DROP COLUMN IF EXISTS fail_stage_key,
            DROP COLUMN IF EXISTS stage_entered_at, DROP COLUMN IF EXISTS cancelled_at"
            .into(),
        "ALTER TABLE projects DROP COLUMN IF EXISTS allow_force_transition".into(),
        "DROP TABLE IF EXISTS project_workflows".into(),
        "ALTER TABLE ticket_types DROP CONSTRAINT IF EXISTS ticket_types_workflow_fkey,
            DROP COLUMN IF EXISTS workflow_id"
            .into(),
        "DROP TABLE IF EXISTS workflow_versions".into(),
        "DROP TABLE IF EXISTS workflows".into(),
        "DELETE FROM notifications WHERE kind IN
            ('review_requested', 'approved', 'rejected', 'decision_needed')"
            .into(),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M2}))"
        ),
        format!("DELETE FROM ticket_events WHERE kind NOT IN ({EVENT_KINDS_M2})"),
        "ALTER TABLE ticket_events DROP CONSTRAINT ticket_events_kind_check".into(),
        format!(
            "ALTER TABLE ticket_events ADD CONSTRAINT ticket_events_kind_check \
             CHECK (kind IN ({EVENT_KINDS_M2}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action NOT IN ({AUDIT_ACTIONS_M2}) \
             OR subject_type NOT IN ({AUDIT_SUBJECTS_M2})"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M2}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M2}))"
        ),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in up() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in down() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
