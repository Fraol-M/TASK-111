-- Add optimistic-concurrency version columns to the remaining mutable finance
-- aggregates so state transitions across intents, payments, and refunds share a
-- uniform lost-update protection story.
--
-- `payments.version` already exists (migration 0008). This migration brings
-- payment_intents and refunds into alignment. Every state transition on these
-- rows must now include `WHERE version = <expected>` and increment it.
ALTER TABLE payment_intents
    ADD COLUMN version INTEGER NOT NULL DEFAULT 0;

ALTER TABLE refunds
    ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
