use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection =
            Connection::open(path).map_err(|error| format!("打开数据库失败: {error}"))?;

        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| format!("启用数据库外键失败: {error}"))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| format!("启用数据库 WAL 模式失败: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("设置数据库等待时间失败: {error}"))?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn connection(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "数据库连接锁已损坏".to_string())
    }
}
