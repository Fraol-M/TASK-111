-- Add user timezone offset so DND windows are evaluated in the member's local time,
-- not the server's local time. Stored as signed integer minutes east of UTC
-- (e.g. +180 = UTC+3, -300 = UTC-5). Default 0 = UTC.
ALTER TABLE member_preferences
    ADD COLUMN timezone_offset_minutes INTEGER NOT NULL DEFAULT 0;
