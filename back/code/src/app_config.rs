// main urls
pub const HOST: &'static str = "0.0.0.0:5000";
pub const SERVICE_URL: &'static str = "http://127.0.0.1:5000";

// threads count
pub const THREADS_COUNT: usize = 3;

// postgres connections count
pub const POSTGRESQL_CONNECTIONS_LIMIT: u32 = 5;

// postgres data model
pub const APP_SCHEMA: &'static str = "routine_app";
pub const USERS_TABLE: &'static str = "customer";
pub const BOARDS_TABLE: &'static str = "board";
pub const TASKS_TABLE: &'static str = "task";

// redis ttls
pub const STORED_DATA_EXPIRATION_TIME: usize = 86_400; // 1 day cache lifetime
pub const USER_DATA_EXPIRATION_TIME: usize = 259_200; // 3 days cache lifetime

// token lifetime
pub const TOKEN_LIFETIME: i64 = 86_400; // 24 hours lifetime
pub const TOKEN_UPDATE_LIFETIME_THRESHOLD: i64 = 64_800; // 18 hours lifetime

// logs
pub const LOGS_CONFIG_FILE: &'static str = "log_config.yml";