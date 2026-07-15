#!/usr/bin/env bash
set -euo pipefail
# Daily backup: pg_dump + PDF directory.
# Configure REMOTE via rclone/rsync as needed.

BACKUP_ROOT="${BACKUP_ROOT:-./backups}"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
DIR="$BACKUP_ROOT/$STAMP"
mkdir -p "$DIR"

echo "Dumping database…"
pg_dump "${DATABASE_URL:-postgres://epistemic:epistemic@localhost:5432/epistemic}" \
  --format=custom --file="$DIR/epistemic.dump"

echo "Copying PDFs…"
if [[ -d "${PDF_DIR:-./data/pdfs}" ]]; then
  tar -czf "$DIR/pdfs.tar.gz" -C "$(dirname "${PDF_DIR:-./data/pdfs}")" "$(basename "${PDF_DIR:-./data/pdfs}")"
fi

echo "Backup written to $DIR"
# Optional offsite:
# rclone copy "$DIR" remote:epistemic-backups/"$STAMP"
