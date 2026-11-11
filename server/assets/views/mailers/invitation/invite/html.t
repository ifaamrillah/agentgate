{#- Variabel: name (alamat email penerima), workspace_name, inviter_name, role_name, link (URL
    undangan lengkap), days (umur link). -#}
{% extends "_layout.html.t" %}
{% block body %}<p style="margin:0 0 16px;">{{ t(key="mail-invitation-body", lang="id", inviter=inviter_name, workspace=workspace_name, role=role_name) }}</p>{% endblock body %}
{% block action_link %}{{ link }}{% endblock action_link %}
{% block action_label %}{{ t(key="mail-invitation-action", lang="id") }}{% endblock action_label %}
{% block expiry %}{{ t(key="mail-invitation-expiry", lang="id", days=days) }}{% endblock expiry %}
