-- Background job execution log (for observability and idempotency)
CREATE TABLE job_runs (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    job_name        TEXT        NOT NULL,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at     TIMESTAMPTZ,
    status          TEXT        NOT NULL DEFAULT 'running',  -- 'running','completed','failed'
    items_processed INTEGER,
    error_detail    TEXT
);

CREATE INDEX job_runs_name_started_idx ON job_runs(job_name, started_at DESC);
