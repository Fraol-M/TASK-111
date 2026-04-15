-- Restore multi-channel enum for rollback
ALTER TYPE notification_channel RENAME TO notification_channel_old;
CREATE TYPE notification_channel AS ENUM ('in_app', 'email', 'sms', 'push');

ALTER TABLE notification_templates
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

ALTER TABLE notifications
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

DROP TYPE notification_channel_old;
