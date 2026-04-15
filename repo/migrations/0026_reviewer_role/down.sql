-- Postgres does not support removing a value from an enum type. Rolling back
-- this migration requires a schema swap (rename old type, create new type
-- without the value, ALTER COLUMN ... USING, drop old type) which is unsafe
-- if any row currently holds the value. Down-migration is intentionally a
-- no-op; rebuild the DB from a snapshot to fully revert.
SELECT 1;
