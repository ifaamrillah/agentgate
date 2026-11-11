{#- Variabel: name (alamat email penerima), workspace_name, inviter_name, role_name, link (URL
    undangan lengkap), days (umur link). -#}
{{ t(key="mail-greeting", lang="id", name=name) }}

{{ t(key="mail-invitation-body", lang="id", inviter=inviter_name, workspace=workspace_name, role=role_name) }}

{{ link }}

{{ t(key="mail-invitation-expiry", lang="id", days=days) }}

{{ t(key="mail-footer", lang="id") }}
