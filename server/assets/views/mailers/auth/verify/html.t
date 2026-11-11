{#- Variabel: name, link (URL verifikasi lengkap), hours (umur link). -#}
{% extends "_layout.html.t" %}
{% block body %}<p style="margin:0 0 16px;">{{ t(key="mail-verify-body", lang="id") }}</p>{% endblock body %}
{% block action_link %}{{ link }}{% endblock action_link %}
{% block action_label %}{{ t(key="mail-verify-action", lang="id") }}{% endblock action_label %}
{% block expiry %}{{ t(key="mail-verify-expiry", lang="id", hours=hours) }}{% endblock expiry %}
