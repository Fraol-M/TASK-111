-- Add tamper-evidence fields to audit_logs for hash-chain integrity verification.
-- row_hash: SHA-256 of (id || action || entity_type || entity_id || created_at || previous_hash)
-- previous_hash: row_hash of the preceding audit_logs entry (NULL for first row)
ALTER TABLE audit_logs ADD COLUMN row_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE audit_logs ADD COLUMN previous_hash TEXT;
