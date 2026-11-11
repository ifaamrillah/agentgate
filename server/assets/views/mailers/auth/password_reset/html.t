{#- Variabel: name, link (URL reset password lengkap), minutes (umur link). -#}
{% extends "_layout.html.t" %}
{% block body %}<p style="margin:0 0 16px;">{{ t(key="mail-password-reset-body", lang="id") }}</p>{% endblock body %}
{% block action_link %}{{ link }}{% endblock action_link %}
{% block action_label %}{{ t(key="mail-password-reset-action", lang="id") }}{% endblock action_label %}
{% block expiry %}{{ t(key="mail-password-reset-expiry", lang="id", minutes=minutes) }}{% endblock expiry %}
