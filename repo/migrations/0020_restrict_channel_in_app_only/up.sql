-- Enforce offline-capable-only policy at schema level: restrict notification_channel
-- to 'in_app' only. Email/sms/push enum values were disabled extension points that
-- never stored data (dispatch always failed and fell back to in_app).
-- When a provider is integrated, add its value back with a new migration.

ALTER TYPE notification_channel RENAME TO notification_channel_old;
CREATE TYPE notification_channel AS ENUM ('in_app');

ALTER TABLE notification_templates
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

ALTER TABLE notifications
    ALTER COLUMN channel TYPE notification_channel
    USING channel::text::notification_channel;

DROP TYPE notification_channel_old;
