-- Evaluation workflow states
CREATE TYPE evaluation_state AS ENUM (
    'draft',
    'open',
    'in_review',
    'completed',
    'cancelled'
);

-- Assignment states (node-level)
CREATE TYPE assignment_state AS ENUM (
    'pending',
    'in_progress',
    'submitted',
    'approved',
    'rejected'
);

-- Evaluation cycles (periodic assessment campaigns)
CREATE TABLE evaluation_cycles (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    description TEXT,
    starts_at   TIMESTAMPTZ NOT NULL,
    ends_at     TIMESTAMPTZ NOT NULL,
    created_by  UUID        REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_cycle_dates CHECK (ends_at > starts_at)
);

-- Individual evaluations within a cycle
CREATE TABLE evaluations (
    id          UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    cycle_id    UUID             REFERENCES evaluation_cycles(id),
    title       TEXT             NOT NULL,
    description TEXT,
    state       evaluation_state NOT NULL DEFAULT 'draft',
    version     INTEGER          NOT NULL DEFAULT 0,
    created_by  UUID             REFERENCES users(id),
    created_at  TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);

-- Assignment of an evaluator to assess a subject within an evaluation
CREATE TABLE evaluation_assignments (
    id            UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    evaluation_id UUID             NOT NULL REFERENCES evaluations(id) ON DELETE CASCADE,
    evaluator_id  UUID             NOT NULL REFERENCES users(id),
    subject_id    UUID             REFERENCES users(id),
    state         assignment_state NOT NULL DEFAULT 'pending',
    due_at        TIMESTAMPTZ,
    created_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    UNIQUE (evaluation_id, evaluator_id)
);

-- Audit trail of actions taken on an assignment (state transitions, comments)
CREATE TABLE evaluation_actions (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    assignment_id UUID        NOT NULL REFERENCES evaluation_assignments(id) ON DELETE CASCADE,
    actor_id      UUID        NOT NULL REFERENCES users(id),
    action_type   TEXT        NOT NULL,
    notes         TEXT,
    payload       JSONB,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
