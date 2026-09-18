mod database;
mod migrations;

pub use database::Database;
pub use migrations::run as run_migrations;

pub fn unix_timestamp() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|error| format!("读取系统时间失败: {error}"))
}
