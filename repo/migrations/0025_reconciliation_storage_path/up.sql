-- Persist the original uploaded reconciliation file path so finance/audit can
-- re-open the source document for any imported row.
--
-- The storage flow writes bytes to `cfg.storage.reconciliation_dir` under a
-- checksum-derived name (`<sha256>.csv`) before parsing, then records the
-- absolute path here. The column is nullable so historical imports (which had
-- no on-disk artifact) remain queryable.
ALTER TABLE reconciliation_imports
    ADD COLUMN storage_path TEXT;
