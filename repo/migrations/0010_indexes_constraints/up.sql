-- ==================== Booking & Inventory Indexes ====================

-- Bookings: fast lookup by member, state, and time
CREATE INDEX bookings_member_id_idx ON bookings(member_id);
CREATE INDEX bookings_state_idx ON bookings(state);
CREATE INDEX bookings_start_at_idx ON bookings(start_at);

-- Inventory holds: find expiring/unreleased holds efficiently
CREATE INDEX inventory_holds_expires_idx
    ON inventory_holds(expires_at)
    WHERE released_at IS NULL;
CREATE INDEX inventory_holds_booking_idx ON inventory_holds(booking_id);
CREATE INDEX inventory_holds_item_idx ON inventory_holds(inventory_item_id);

-- Inventory ledger: lookup by item
CREATE INDEX inventory_ledger_item_idx ON inventory_ledger(inventory_item_id);

-- ==================== Member & Loyalty Indexes ====================

CREATE INDEX points_ledger_user_id_idx ON points_ledger(user_id);
CREATE INDEX wallet_ledger_user_id_idx ON wallet_ledger(user_id);
CREATE INDEX member_tier_history_user_idx ON member_tier_history(user_id);
CREATE INDEX blacklist_events_user_idx ON blacklist_events(user_id);

-- ==================== Notification Indexes ====================

CREATE INDEX notifications_user_state_idx ON notifications(user_id, delivery_state);
CREATE INDEX notifications_dnd_idx ON notifications(dnd_suppressed) WHERE dnd_suppressed = TRUE;
CREATE INDEX dnd_queue_scheduled_idx
    ON dnd_queue(scheduled_deliver_at)
    WHERE processed_at IS NULL;
CREATE INDEX notification_attempts_notif_idx ON notification_attempts(notification_id);

-- ==================== Asset Indexes ====================

CREATE INDEX asset_versions_asset_id_idx ON asset_versions(asset_id);
CREATE INDEX assets_status_idx ON assets(status);
CREATE INDEX assets_location_idx ON assets(location) WHERE location IS NOT NULL;

-- ==================== Evaluation Indexes ====================

CREATE INDEX evaluations_cycle_id_idx ON evaluations(cycle_id);
CREATE INDEX evaluation_assignments_eval_idx ON evaluation_assignments(evaluation_id);
CREATE INDEX evaluation_assignments_evaluator_idx ON evaluation_assignments(evaluator_id);
CREATE INDEX evaluation_actions_assignment_idx ON evaluation_actions(assignment_id);

-- ==================== Payment Indexes ====================

-- Find open intents that need timeout processing
CREATE INDEX payment_intents_expires_idx
    ON payment_intents(expires_at)
    WHERE state = 'open';
CREATE INDEX payments_booking_id_idx ON payments(booking_id);
CREATE INDEX payments_member_id_idx ON payments(member_id);
CREATE INDEX refunds_payment_id_idx ON refunds(payment_id);
CREATE INDEX reconciliation_rows_import_idx ON reconciliation_rows(import_id);

-- ==================== Audit Indexes ====================

CREATE INDEX audit_logs_entity_idx ON audit_logs(entity_type, entity_id);
CREATE INDEX audit_logs_actor_idx ON audit_logs(actor_user_id);
CREATE INDEX audit_logs_created_at_idx ON audit_logs(created_at DESC);
CREATE INDEX audit_logs_correlation_idx ON audit_logs(correlation_id);

-- ==================== Group Indexes ====================

CREATE INDEX group_members_user_idx ON group_members(user_id);
CREATE INDEX group_messages_thread_idx ON group_messages(thread_id, created_at DESC);
CREATE INDEX group_message_receipts_user_idx ON group_message_receipts(user_id);

-- ==================== Restock Alerts ====================

CREATE INDEX restock_alerts_item_idx
    ON restock_alerts(inventory_item_id)
    WHERE acknowledged_at IS NULL;
