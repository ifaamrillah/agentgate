{#- Kerangka HTML semua email. Block: body (isi), action_link (URL tombol), action_label (teks
    tombol), expiry (catatan di bawah tombol). Variabel: name (string). Style inline karena
    klien email membuang <style> dan CSS eksternal. -#}
<!doctype html>
<html lang="id">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head>
<body style="margin:0;padding:24px;background:#f4f4f5;font-family:Arial,Helvetica,sans-serif;color:#18181b;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background:#ffffff;border-radius:8px;padding:32px;">
<tr><td>
<p style="margin:0 0 24px;font-weight:bold;font-size:18px;">{{ t(key="app-name", lang="id") }}</p>
<p style="margin:0 0 16px;">{{ t(key="mail-greeting", lang="id", name=name) }}</p>
{% block body %}{% endblock body %}
<p style="margin:24px 0;"><a href="{% block action_link %}{% endblock action_link %}" style="display:inline-block;background:#18181b;color:#ffffff;text-decoration:none;padding:10px 16px;border-radius:6px;">{% block action_label %}{% endblock action_label %}</a></p>
<p style="margin:0 0 16px;font-size:14px;color:#52525b;">{% block expiry %}{% endblock expiry %}</p>
<p style="margin:0;font-size:12px;color:#71717a;">{{ t(key="mail-footer", lang="id") }}</p>
</td></tr>
</table>
</td></tr></table>
</body>
</html>
