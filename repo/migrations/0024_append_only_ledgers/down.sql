DROP TRIGGER IF EXISTS inventory_ledger_append_only ON inventory_ledger;
DROP TRIGGER IF EXISTS wallet_ledger_append_only    ON wallet_ledger;
DROP TRIGGER IF EXISTS points_ledger_append_only    ON points_ledger;
DROP TRIGGER IF EXISTS audit_logs_append_only       ON audit_logs;
DROP FUNCTION IF EXISTS reject_ledger_mutation();
