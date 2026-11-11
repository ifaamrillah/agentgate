{#- Variabel: name, link (URL reset password lengkap), login_link (URL halaman masuk),
    minutes (umur link). -#}
{% extends "_layout.html.t" %}
{% block body %}<p style="margin:0 0 16px;">{{ t(key="mail-already-registered-body", lang="id") }}</p>
<p style="margin:0 0 16px;">{{ t(key="mail-already-registered-login", lang="id") }} <a href="{{ login_link }}">{{ login_link }}</a></p>{% endblock body %}
{% block action_link %}{{ link }}{% endblock action_link %}
{% block action_label %}{{ t(key="mail-already-registered-action", lang="id") }}{% endblock action_label %}
{% block expiry %}{{ t(key="mail-already-registered-expiry", lang="id", minutes=minutes) }}{% endblock expiry %}
