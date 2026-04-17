use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub encryption: EncryptionConfig,
    pub jwt: JwtConfig,
    pub jobs: JobsConfig,
    pub booking: BookingConfig,
    pub payment: PaymentConfig,
    pub dnd: DndConfig,
    pub backup: BackupConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub bootstrap: BootstrapConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EncryptionConfig {
    pub key_hex: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry_seconds: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JobsConfig {
    pub hold_expiry_interval_secs: u64,
    pub payment_timeout_interval_secs: u64,
    pub reminder_interval_secs: u64,
    pub dnd_resolve_interval_secs: u64,
    pub zero_qty_interval_secs: u64,
    pub tier_recalc_hour: u32,
    pub backup_hour: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BookingConfig {
    pub hold_timeout_minutes: i64,
    /// Inventory deduction strategy:
    /// - "hold" (default): pre-decrement via reservation on booking creation;
    ///   restore on cancel/expiry; transitions Draft → Held.
    /// - "immediate": decrement directly on booking creation with no hold row;
    ///   transitions Draft → Confirmed in a single step. `change_booking` applies
    ///   net deltas (restore old qty → deduct new qty) under the same transaction.
    #[serde(default = "default_inventory_strategy")]
    pub inventory_strategy: String,
}

fn default_inventory_strategy() -> String {
    "hold".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentConfig {
    pub intent_timeout_minutes: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DndConfig {
    pub start_hour: u32,
    pub end_hour: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BackupConfig {
    pub dir: String,
    /// UTC offset in minutes for local-time backup scheduling.
    /// Default: 0 (UTC). Set to e.g. -300 for US Eastern, 60 for CET.
    #[serde(default)]
    pub timezone_offset_minutes: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub reconciliation_dir: String,
    pub attachments_dir: String,
    pub max_upload_bytes: usize,
}

/// Operator-controlled gate on which notification channels may be selected.
///
/// The DB enum intentionally models all four channels (`in_app`, `email`,
/// `sms`, `push`) so the schema can express future provider integrations,
/// but only channels listed in `enabled_channels` (parsed from a
/// comma-separated string) may be chosen when creating templates or
/// updating member preferences. Any attempt to select a disabled channel
/// is rejected at the API layer with a 422 rather than silently falling
/// through to the in-app fallback path.
///
/// Default profile: `in_app` only. Operators wiring a real provider must
/// add the corresponding channel explicitly via
/// `APP__NOTIFICATIONS__ENABLED_CHANNELS=in_app,email`.
///
/// The raw value is held as a `String` so the `config` crate's environment
/// source can deserialize it without a custom list separator; `enabled_channels()`
/// derives the parsed `Vec<String>` on demand.
#[derive(Debug, Deserialize, Clone)]
pub struct NotificationsConfig {
    #[serde(default = "default_enabled_channels_raw", rename = "enabled_channels")]
    enabled_channels_raw: String,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self { enabled_channels_raw: default_enabled_channels_raw() }
    }
}

fn default_enabled_channels_raw() -> String {
    "in_app".to_string()
}

impl NotificationsConfig {
    /// Parse the raw comma-separated allow-list into trimmed lowercase tokens.
    pub fn enabled_channels(&self) -> Vec<String> {
        self.enabled_channels_raw
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Case-insensitive membership check against the configured allow-list.
    pub fn channel_is_enabled(&self, channel: &str) -> bool {
        let needle = channel.trim().to_ascii_lowercase();
        self.enabled_channels()
            .iter()
            .any(|c| c == &needle)
    }
}

/// Startup-seeding behavior. Controlled via env so the same binary can run
/// either a dev profile (auto-seed demo users on first boot) or a production
/// profile (no seeding — operator creates admin accounts manually via the
/// admin API).
///
/// The seeder is idempotent (`ON CONFLICT (username) DO NOTHING`) so restarting
/// with the flag on does not overwrite modified passwords or roles — it only
/// fills in users that are not yet present.
#[derive(Debug, Deserialize, Clone)]
pub struct BootstrapConfig {
    /// When true, the application seeds a fixed set of demo users (`admin`,
    /// `ops`, `finance`, `asset_mgr`, `evaluator`, `reviewer`, `member`) with
    /// the canonical demo password on startup. Default: false.
    ///
    /// This flag MUST be false in production — demo credentials are
    /// well-known and would be a privilege-escalation vector if left on.
    /// `.env.example` sets it true so `docker-compose up` gives a reviewer a
    /// fully-usable instance without any manual DB setup.
    #[serde(default)]
    pub seed_demo_users: bool,
    /// Password used for seeded demo users. Defaults to `Test1234!` which
    /// matches the value baked into integration tests. Operators who want a
    /// different dev password can set `APP__BOOTSTRAP__DEMO_PASSWORD`.
    #[serde(default = "default_demo_password")]
    pub demo_password: String,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            seed_demo_users: false,
            demo_password: default_demo_password(),
        }
    }
}

fn default_demo_password() -> String {
    "Test1234!".to_string()
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder()
            .add_source(
                config::Environment::with_prefix("APP")
                    .separator("__")
                    .try_parsing(true),
            );

        // Preserve all-digit hex keys as strings. Without this explicit
        // override, the config crate may coerce APP__ENCRYPTION__KEY_HEX into
        // a number before deserialization, which breaks hex decoding in tests.
        if let Ok(key_hex) = std::env::var("APP__ENCRYPTION__KEY_HEX") {
            builder = builder.set_override("encryption.key_hex", key_hex)?;
        }

        let cfg = builder.build()?;
        cfg.try_deserialize()
    }
}
