-- Enum extension for future channel providers (email, sms, push).
-- OFFLINE-CAPABLE POLICY: Only 'in_app' is operational. The additional enum
-- values are disabled extension points — dispatch_to_channel returns Err for
-- non-in_app channels and an in_app fallback notification is created automatically.
-- To enable a channel, wire the corresponding provider in dispatch_to_channel.
ALTER TYPE notification_channel RENAME TO notification_channel_old;
CREATE TYPE notification_channel AS ENUM ('in_app', 'email', 'sms', 'push');

ALTER TABLE notification_templates
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

ALTER TABLE notifications
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

DROP TYPE notification_channel_old;
