//! Setiap kunci terjemahan yang dipakai template dan kode punya entri di Fluent.

use std::{collections::HashSet, path::Path};

use agentgate_server::{
    common::text,
    models::{
        job_titles, permission::Permission, project_grants, projects, repos, roles, teams, users,
        workspaces,
    },
    services::{
        auth::{oauth, password, password_change},
        workspaces::{accept_invite, deletion, invite, membership, require_two_factor},
    },
    views::{self, account, auth, errors::MESSAGE_KEYS},
};
use sea_orm::Iterable;

/// Kunci dari semua file `.ftl` locale `id` (loader memuat seluruh isi folder).
fn fluent_keys() -> HashSet<String> {
    let mut src = String::new();
    for entry in std::fs::read_dir("assets/i18n/id").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ftl") {
            src.push_str(&std::fs::read_to_string(&path).unwrap());
            src.push('\n');
        }
    }
    src.lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim().to_string()))
        .filter(|k| !k.starts_with('#') && !k.starts_with(' '))
        .collect()
}

fn template_keys(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            template_keys(&path, out);
        } else if path.extension().is_some_and(|e| e == "html" || e == "t") {
            let src = std::fs::read_to_string(&path).unwrap();
            for part in src.split("t(key=\"").skip(1) {
                if let Some(end) = part.find('"') {
                    out.push((path.display().to_string(), part[..end].to_string()));
                }
            }
        }
    }
}

#[test]
fn every_literal_template_key_has_a_translation() {
    let keys = fluent_keys();
    let mut used = Vec::new();
    template_keys(Path::new("assets/views"), &mut used);
    assert!(!used.is_empty());
    let missing: Vec<_> = used.iter().filter(|(_, k)| !keys.contains(k)).collect();
    assert!(missing.is_empty(), "kunci tanpa terjemahan: {missing:?}");
}

#[test]
fn every_error_message_key_has_title_and_body() {
    let keys = fluent_keys();
    for key in MESSAGE_KEYS {
        for suffix in ["-title", "-body"] {
            assert!(keys.contains(&format!("{key}{suffix}")), "{key}{suffix}");
        }
    }
}

#[test]
fn every_validation_error_code_has_a_message() {
    let keys = fluent_keys();
    let codes = [
        text::ERROR_CODES,
        users::ERROR_CODES,
        workspaces::ERROR_CODES,
        roles::ERROR_CODES,
        job_titles::ERROR_CODES,
        teams::ERROR_CODES,
        projects::ERROR_CODES,
        project_grants::ERROR_CODES,
        repos::ERROR_CODES,
        password::ERROR_CODES,
        password_change::ERROR_CODES,
        invite::ERROR_CODES,
        accept_invite::ERROR_CODES,
        membership::ERROR_CODES,
        deletion::ERROR_CODES,
        require_two_factor::ERROR_CODES,
        agentgate_server::services::workspaces::teams::ERROR_CODES,
        agentgate_server::services::projects::logo::ERROR_CODES,
        agentgate_server::models::tickets::ERROR_CODES,
        agentgate_server::models::ticket_types::ERROR_CODES,
        agentgate_server::models::ticket_links::ERROR_CODES,
        agentgate_server::models::ticket_attachments::ERROR_CODES,
        agentgate_server::models::ticket_comments::ERROR_CODES,
        agentgate_server::models::ticket_code::ERROR_CODES,
        agentgate_server::models::custom_fields::ERROR_CODES,
    ]
    .concat();
    let missing: Vec<_> = codes.iter().filter(|c| !keys.contains(**c)).collect();
    assert!(missing.is_empty(), "kode error tanpa pesan: {missing:?}");
}

#[test]
fn every_permission_has_label_and_description() {
    let keys = fluent_keys();
    for permission in Permission::iter() {
        let (label, description) = views::roles::permission_keys(permission);
        assert!(keys.contains(label), "{label}");
        assert!(keys.contains(description), "{description}");
    }
}

#[test]
fn every_auth_message_key_has_a_translation() {
    let keys = fluent_keys();
    let missing: Vec<_> = [
        auth::MESSAGE_KEYS,
        account::MESSAGE_KEYS,
        views::workspaces::MESSAGE_KEYS,
        views::roles::MESSAGE_KEYS,
        views::job_titles::MESSAGE_KEYS,
        views::members::MESSAGE_KEYS,
        views::teams::MESSAGE_KEYS,
        views::projects::MESSAGE_KEYS,
        views::tickets::MESSAGE_KEYS,
        oauth::ERROR_CODES,
    ]
    .concat()
    .into_iter()
    .filter(|k| !keys.contains(*k))
    .collect();
    assert!(missing.is_empty(), "kunci tanpa terjemahan: {missing:?}");
}

#[test]
fn every_repo_provider_has_a_label() {
    let keys = fluent_keys();
    for provider in repos::RepoProvider::iter() {
        let key = views::projects::provider_key(Some(provider));
        assert!(keys.contains(key), "{key}");
    }
}

#[test]
fn every_audit_action_subject_and_group_has_a_label() {
    use agentgate_server::models::audit_events::{AuditAction, AuditGroup, AuditSubject};
    let keys = fluent_keys();
    let mut missing = Vec::new();
    for action in AuditAction::iter() {
        missing.push(views::audit::action_key(action));
    }
    for subject in AuditSubject::iter() {
        missing.push(views::audit::subject_key(subject));
    }
    for group in AuditGroup::iter() {
        missing.push(views::audit::group_key(group));
    }
    missing.extend(views::audit::MESSAGE_KEYS.iter().map(|k| (*k).to_string()));
    missing.retain(|k| !keys.contains(k));
    assert!(missing.is_empty(), "kunci tanpa terjemahan: {missing:?}");
}

#[test]
fn every_ticket_enum_and_view_key_has_a_translation() {
    use agentgate_server::models::{notifications::NotificationKind, ticket_events::EventKind};
    let keys = fluent_keys();
    let mut used: Vec<String> = Vec::new();
    for kind in EventKind::iter() {
        let value = sea_orm::ActiveEnum::to_value(&kind);
        used.push(format!("ticket-event-{}", value.replace('_', "-")));
    }
    for kind in NotificationKind::iter() {
        used.push(format!(
            "notification-{}",
            sea_orm::ActiveEnum::to_value(&kind)
        ));
    }
    for s in views::tickets::status_options() {
        used.push(s.label_key.to_string());
    }
    for p in views::tickets::priority_options() {
        used.push(p.label_key.to_string());
    }
    for o in views::tickets::field_kind_options()
        .into_iter()
        .chain(views::tickets::scope_options())
    {
        used.push(o.name);
    }
    for t in views::tickets::code_tokens() {
        used.push(t.label_key.to_string());
    }
    for field in agentgate_server::views::tickets::TICKET_FIELDS {
        used.push(format!(
            "ticket-field-{}",
            field.trim_end_matches("_id").replace('_', "-")
        ));
    }
    used.push("ticket-field-custom-values".into());
    used.retain(|k| !keys.contains(k));
    assert!(used.is_empty(), "kunci tanpa terjemahan: {used:?}");
}
