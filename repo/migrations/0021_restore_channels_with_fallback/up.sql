-- Restore multi-channel enum to support configurable channel delivery with
-- graceful in_app fallback. Non-in_app channels are dispatch-gated at runtime:
-- when a provider is not configured, dispatch returns an error and the service
-- automatically creates an in_app fallback notification + attempt record.
ALTER TYPE notification_channel RENAME TO notification_channel_old;
CREATE TYPE notification_channel AS ENUM ('in_app', 'email', 'sms', 'push');

ALTER TABLE notification_templates
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

ALTER TABLE notifications
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

DROP TYPE notification_channel_old;
