-- Remove unimplemented notification channels; only 'in_app' is delivered.
-- Email/SMS/Push delivery is not wired — leaving the enum values created the
-- false impression that those channels were functional.
ALTER TYPE notification_channel RENAME TO notification_channel_old;
CREATE TYPE notification_channel AS ENUM ('in_app');

ALTER TABLE notification_templates
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

ALTER TABLE notifications
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

DROP TYPE notification_channel_old;
