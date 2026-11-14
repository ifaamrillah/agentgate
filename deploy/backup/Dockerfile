# Container backup (M12): klien Postgres 18 (pg_dump/pg_restore/psql) + rclone untuk S3.
FROM postgres:18-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends rclone ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY backup.sh retention.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/backup.sh
USER postgres
ENTRYPOINT ["backup.sh"]
CMD ["loop"]
