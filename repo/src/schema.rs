// @generated — matches models. Regenerate with: diesel print-schema > src/schema.rs

// Custom PostgreSQL ENUM types for use in table! macros
pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "user_role"))]
    pub struct UserRole;
    impl diesel::query_builder::QueryId for UserRole {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "user_status"))]
    pub struct UserStatus;
    impl diesel::query_builder::QueryId for UserStatus {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "member_tier"))]
    pub struct MemberTier;
    impl diesel::query_builder::QueryId for MemberTier {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "blacklist_reason"))]
    pub struct BlacklistReason;
    impl diesel::query_builder::QueryId for BlacklistReason {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "points_txn_type"))]
    pub struct PointsTxnType;
    impl diesel::query_builder::QueryId for PointsTxnType {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "wallet_txn_type"))]
    pub struct WalletTxnType;
    impl diesel::query_builder::QueryId for WalletTxnType {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "publish_status"))]
    pub struct PublishStatus;
    impl diesel::query_builder::QueryId for PublishStatus {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "booking_state"))]
    pub struct BookingState;
    impl diesel::query_builder::QueryId for BookingState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "notification_channel"))]
    pub struct NotificationChannel;
    impl diesel::query_builder::QueryId for NotificationChannel {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "delivery_state"))]
    pub struct DeliveryState;
    impl diesel::query_builder::QueryId for DeliveryState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "template_trigger"))]
    pub struct TemplateTrigger;
    impl diesel::query_builder::QueryId for TemplateTrigger {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "asset_status"))]
    pub struct AssetStatus;
    impl diesel::query_builder::QueryId for AssetStatus {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "depreciation_method"))]
    pub struct DepreciationMethod;
    impl diesel::query_builder::QueryId for DepreciationMethod {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "evaluation_state"))]
    pub struct EvaluationState;
    impl diesel::query_builder::QueryId for EvaluationState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "assignment_state"))]
    pub struct AssignmentState;
    impl diesel::query_builder::QueryId for AssignmentState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "intent_state"))]
    pub struct IntentState;
    impl diesel::query_builder::QueryId for IntentState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "payment_state"))]
    pub struct PaymentState;
    impl diesel::query_builder::QueryId for PaymentState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "refund_state"))]
    pub struct RefundState;
    impl diesel::query_builder::QueryId for RefundState {
        type QueryId = Self;
        const HAS_STATIC_QUERY_ID: bool = true;
    }
}

diesel::table! {
    use diesel::sql_types::*;

    audit_logs (id) {
        id -> Uuid,
        correlation_id -> Nullable<Text>,
        actor_user_id -> Nullable<Uuid>,
        action -> Text,
        entity_type -> Text,
        entity_id -> Text,
        old_value -> Nullable<Jsonb>,
        new_value -> Nullable<Jsonb>,
        metadata -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        row_hash -> Text,
        previous_hash -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    auth_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        token_hash -> Text,
        expires_at -> Timestamptz,
        revoked_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    idempotency_keys (id) {
        id -> Uuid,
        key_value -> Text,
        request_hash -> Text,
        response_status -> Nullable<SmallInt>,
        response_body -> Nullable<Text>,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    password_history (id) {
        id -> Uuid,
        user_id -> Uuid,
        password_hash -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::{UserRole, UserStatus};

    users (id) {
        id -> Uuid,
        username -> Varchar,
        password_hash -> Text,
        role -> UserRole,
        status -> UserStatus,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::{MemberTier, BlacklistReason};

    members (user_id) {
        user_id -> Uuid,
        tier -> MemberTier,
        points_balance -> Integer,
        wallet_balance -> Text,
        blacklist_flag -> Bool,
        blacklist_reason -> Nullable<BlacklistReason>,
        blacklisted_at -> Nullable<Timestamptz>,
        redemption_frozen_until -> Nullable<Timestamptz>,
        rolling_12m_spend -> Int8,
        version -> Integer,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::MemberTier;

    member_tier_history (id) {
        id -> Uuid,
        user_id -> Uuid,
        from_tier -> Nullable<MemberTier>,
        to_tier -> MemberTier,
        reason -> Nullable<Text>,
        actor_user_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::PointsTxnType;

    points_ledger (id) {
        id -> Uuid,
        user_id -> Uuid,
        txn_type -> PointsTxnType,
        delta -> Integer,
        balance_after -> Integer,
        reference_id -> Nullable<Uuid>,
        note -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::WalletTxnType;

    wallet_ledger (id) {
        id -> Uuid,
        user_id -> Uuid,
        txn_type -> WalletTxnType,
        delta_cents -> Int8,
        balance_after_cents -> Int8,
        reference_id -> Nullable<Uuid>,
        note -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    member_preferences (user_id) {
        user_id -> Uuid,
        notification_opt_out -> Jsonb,
        preferred_channel -> Text,
        timezone_offset_minutes -> Integer,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::BlacklistReason;

    blacklist_events (id) {
        id -> Uuid,
        user_id -> Uuid,
        action -> Text,
        reason -> Nullable<BlacklistReason>,
        duration_days -> Nullable<Integer>,
        note -> Nullable<Text>,
        actor_user_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    pickup_points (id) {
        id -> Uuid,
        name -> Text,
        address -> Nullable<Text>,
        active -> Bool,
        created_at -> Timestamptz,
        cutoff_hours -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    delivery_zones (id) {
        id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        active -> Bool,
        created_at -> Timestamptz,
        cutoff_hours -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::PublishStatus;

    inventory_items (id) {
        id -> Uuid,
        sku -> Text,
        name -> Text,
        description -> Nullable<Text>,
        available_qty -> Integer,
        safety_stock -> Integer,
        publish_status -> PublishStatus,
        pickup_point_id -> Nullable<Uuid>,
        zone_id -> Nullable<Uuid>,
        cutoff_hours -> Integer,
        version -> Integer,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    inventory_holds (id) {
        id -> Uuid,
        inventory_item_id -> Uuid,
        booking_id -> Nullable<Uuid>,
        quantity -> Integer,
        expires_at -> Timestamptz,
        released_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    inventory_ledger (id) {
        id -> Uuid,
        inventory_item_id -> Uuid,
        delta -> Integer,
        qty_after -> Integer,
        reason -> Text,
        correlation_id -> Nullable<Text>,
        actor_user_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    restock_alerts (id) {
        id -> Uuid,
        inventory_item_id -> Uuid,
        triggered_qty -> Integer,
        triggered_at -> Timestamptz,
        acknowledged_at -> Nullable<Timestamptz>,
        acknowledged_by -> Nullable<Uuid>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::BookingState;

    bookings (id) {
        id -> Uuid,
        member_id -> Uuid,
        state -> BookingState,
        start_at -> Timestamptz,
        end_at -> Timestamptz,
        inventory_hold_expires_at -> Nullable<Timestamptz>,
        change_reason -> Nullable<Text>,
        pickup_point_id -> Nullable<Uuid>,
        zone_id -> Nullable<Uuid>,
        total_cents -> Int8,
        version -> Integer,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    booking_items (id) {
        id -> Uuid,
        booking_id -> Uuid,
        inventory_item_id -> Uuid,
        quantity -> Integer,
        unit_price_cents -> Int8,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::BookingState;

    booking_status_history (id) {
        id -> Uuid,
        booking_id -> Uuid,
        from_state -> Nullable<BookingState>,
        to_state -> BookingState,
        reason -> Nullable<Text>,
        actor_user_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::{TemplateTrigger, NotificationChannel};

    notification_templates (id) {
        id -> Uuid,
        name -> Text,
        trigger_type -> TemplateTrigger,
        channel -> NotificationChannel,
        subject_template -> Nullable<Text>,
        body_template -> Text,
        variable_schema -> Nullable<Jsonb>,
        is_critical -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::{TemplateTrigger, NotificationChannel, DeliveryState};

    notifications (id) {
        id -> Uuid,
        user_id -> Uuid,
        template_id -> Nullable<Uuid>,
        trigger_type -> TemplateTrigger,
        channel -> NotificationChannel,
        subject -> Nullable<Text>,
        body -> Text,
        payload_hash -> Text,
        delivery_state -> DeliveryState,
        dnd_suppressed -> Bool,
        read_at -> Nullable<Timestamptz>,
        reference_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    notification_attempts (id) {
        id -> Uuid,
        notification_id -> Uuid,
        attempted_at -> Timestamptz,
        succeeded -> Bool,
        error_detail -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    dnd_queue (id) {
        id -> Uuid,
        notification_id -> Uuid,
        user_id -> Uuid,
        scheduled_deliver_at -> Timestamptz,
        processed_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    group_threads (id) {
        id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    group_members (id) {
        id -> Uuid,
        thread_id -> Uuid,
        user_id -> Uuid,
        joined_at -> Timestamptz,
        removed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    group_messages (id) {
        id -> Uuid,
        thread_id -> Uuid,
        sender_id -> Uuid,
        body -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    group_message_receipts (id) {
        id -> Uuid,
        message_id -> Uuid,
        user_id -> Uuid,
        read_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::{AssetStatus, DepreciationMethod};

    assets (id) {
        id -> Uuid,
        asset_code -> Text,
        name -> Text,
        description -> Nullable<Text>,
        status -> AssetStatus,
        procurement_cost -> Text,
        depreciation_method -> DepreciationMethod,
        useful_life_years -> Nullable<Integer>,
        purchase_date -> Nullable<Date>,
        location -> Nullable<Text>,
        version -> Integer,
        classification -> Nullable<Text>,
        brand -> Nullable<Text>,
        model -> Nullable<Text>,
        owner_unit -> Nullable<Text>,
        responsible_user_id -> Nullable<Uuid>,
        useful_life_months -> Nullable<Integer>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    asset_versions (id) {
        id -> Uuid,
        asset_id -> Uuid,
        version_no -> Integer,
        snapshot_json -> Jsonb,
        created_by -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    asset_attachments (id) {
        id -> Uuid,
        asset_id -> Uuid,
        file_name -> Text,
        stored_name -> Text,
        mime_type -> Text,
        size_bytes -> Int8,
        uploaded_by -> Uuid,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    evaluation_cycles (id) {
        id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        starts_at -> Timestamptz,
        ends_at -> Timestamptz,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::EvaluationState;

    evaluations (id) {
        id -> Uuid,
        cycle_id -> Nullable<Uuid>,
        title -> Text,
        description -> Nullable<Text>,
        state -> EvaluationState,
        version -> Integer,
        created_by -> Uuid,
        participant_scope -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::AssignmentState;

    evaluation_assignments (id) {
        id -> Uuid,
        evaluation_id -> Uuid,
        evaluator_id -> Uuid,
        subject_id -> Nullable<Uuid>,
        state -> AssignmentState,
        due_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    evaluation_actions (id) {
        id -> Uuid,
        assignment_id -> Uuid,
        actor_id -> Uuid,
        action_type -> Text,
        notes -> Nullable<Text>,
        payload -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::IntentState;

    payment_intents (id) {
        id -> Uuid,
        booking_id -> Nullable<Uuid>,
        member_id -> Uuid,
        amount_cents -> Int8,
        state -> IntentState,
        idempotency_key -> Text,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        tax_cents -> Int8,
        version -> Integer,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::PaymentState;

    payments (id) {
        id -> Uuid,
        intent_id -> Uuid,
        member_id -> Uuid,
        booking_id -> Nullable<Uuid>,
        amount_cents -> Int8,
        payment_method -> Text,
        state -> PaymentState,
        idempotency_key -> Text,
        external_reference -> Nullable<Text>,
        version -> Integer,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        tax_cents -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::schema::sql_types::RefundState;

    refunds (id) {
        id -> Uuid,
        payment_id -> Uuid,
        amount_cents -> Int8,
        reason -> Nullable<Text>,
        state -> RefundState,
        idempotency_key -> Text,
        requested_by -> Uuid,
        approved_by -> Nullable<Uuid>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        version -> Integer,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    payment_adjustments (id) {
        id -> Uuid,
        payment_id -> Uuid,
        amount_cents -> Int8,
        reason -> Text,
        created_by -> Uuid,
        state -> Text,
        approved_by -> Nullable<Uuid>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    reconciliation_imports (id) {
        id -> Uuid,
        file_name -> Text,
        file_checksum -> Text,
        status -> Text,
        total_rows -> Integer,
        matched_rows -> Integer,
        unmatched_rows -> Integer,
        imported_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        storage_path -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    reconciliation_rows (id) {
        id -> Uuid,
        import_id -> Uuid,
        external_reference -> Text,
        external_amount_cents -> Int8,
        payment_id -> Nullable<Uuid>,
        internal_amount_cents -> Nullable<Int8>,
        discrepancy_cents -> Nullable<Int8>,
        status -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    job_runs (id) {
        id -> Uuid,
        job_name -> Text,
        started_at -> Timestamptz,
        finished_at -> Nullable<Timestamptz>,
        status -> Text,
        items_processed -> Nullable<Integer>,
        error_detail -> Nullable<Text>,
    }
}

// Foreign key relationships
diesel::joinable!(auth_sessions -> users (user_id));
diesel::joinable!(password_history -> users (user_id));
diesel::joinable!(members -> users (user_id));
diesel::joinable!(member_tier_history -> users (user_id));
diesel::joinable!(points_ledger -> users (user_id));
diesel::joinable!(wallet_ledger -> users (user_id));
diesel::joinable!(member_preferences -> users (user_id));
diesel::joinable!(blacklist_events -> users (user_id));
diesel::joinable!(bookings -> users (member_id));
diesel::joinable!(bookings -> pickup_points (pickup_point_id));
diesel::joinable!(bookings -> delivery_zones (zone_id));
diesel::joinable!(inventory_items -> pickup_points (pickup_point_id));
diesel::joinable!(inventory_items -> delivery_zones (zone_id));
diesel::joinable!(inventory_holds -> inventory_items (inventory_item_id));
diesel::joinable!(inventory_holds -> bookings (booking_id));
diesel::joinable!(booking_items -> bookings (booking_id));
diesel::joinable!(booking_items -> inventory_items (inventory_item_id));
diesel::joinable!(booking_status_history -> bookings (booking_id));
diesel::joinable!(inventory_ledger -> inventory_items (inventory_item_id));
diesel::joinable!(restock_alerts -> inventory_items (inventory_item_id));
diesel::joinable!(notifications -> users (user_id));
diesel::joinable!(notifications -> notification_templates (template_id));
diesel::joinable!(notification_attempts -> notifications (notification_id));
diesel::joinable!(dnd_queue -> notifications (notification_id));
diesel::joinable!(dnd_queue -> users (user_id));
diesel::joinable!(group_members -> group_threads (thread_id));
diesel::joinable!(group_members -> users (user_id));
diesel::joinable!(group_messages -> group_threads (thread_id));
diesel::joinable!(group_message_receipts -> group_messages (message_id));
diesel::joinable!(group_message_receipts -> users (user_id));
diesel::joinable!(asset_versions -> assets (asset_id));
diesel::joinable!(asset_attachments -> assets (asset_id));
diesel::joinable!(evaluations -> evaluation_cycles (cycle_id));
diesel::joinable!(evaluation_assignments -> evaluations (evaluation_id));
diesel::joinable!(evaluation_actions -> evaluation_assignments (assignment_id));
diesel::joinable!(payment_intents -> bookings (booking_id));
diesel::joinable!(payments -> payment_intents (intent_id));
diesel::joinable!(payments -> bookings (booking_id));
diesel::joinable!(payments -> users (member_id));
diesel::joinable!(refunds -> payments (payment_id));
diesel::joinable!(payment_adjustments -> payments (payment_id));
diesel::joinable!(reconciliation_rows -> reconciliation_imports (import_id));
diesel::joinable!(reconciliation_rows -> payments (payment_id));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    auth_sessions,
    password_history,
    idempotency_keys,
    audit_logs,
    members,
    member_tier_history,
    points_ledger,
    wallet_ledger,
    member_preferences,
    blacklist_events,
    pickup_points,
    delivery_zones,
    inventory_items,
    inventory_holds,
    inventory_ledger,
    restock_alerts,
    bookings,
    booking_items,
    booking_status_history,
    notification_templates,
    notifications,
    notification_attempts,
    dnd_queue,
    group_threads,
    group_members,
    group_messages,
    group_message_receipts,
    assets,
    asset_versions,
    asset_attachments,
    evaluation_cycles,
    evaluations,
    evaluation_assignments,
    evaluation_actions,
    payment_intents,
    payments,
    refunds,
    payment_adjustments,
    reconciliation_imports,
    reconciliation_rows,
    job_runs,
);
