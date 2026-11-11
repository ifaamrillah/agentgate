{#- Variabel: name, link (URL reset password lengkap), login_link (URL halaman masuk),
    minutes (umur link). -#}
{{ t(key="mail-greeting", lang="id", name=name) }}

{{ t(key="mail-already-registered-body", lang="id") }}

{{ link }}

{{ t(key="mail-already-registered-expiry", lang="id", minutes=minutes) }}

{{ t(key="mail-already-registered-login", lang="id") }} {{ login_link }}

{{ t(key="mail-footer", lang="id") }}
