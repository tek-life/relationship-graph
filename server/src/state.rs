use crate::security::auth::TokenStore;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub tokens: Mutex<TokenStore>,
    pub data_dir: PathBuf,
    /// 正在进行上下文压缩的会话 id 集合（内存压缩竞态标记）：
    /// 并发 add_message 同时越过压缩阈值时只触发一次压缩，其余跳过，
    /// 避免重复压缩误删新消息
    pub compressing_sessions: Mutex<HashSet<String>>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(None),
            tokens: Mutex::new(TokenStore::default()),
            data_dir,
            compressing_sessions: Mutex::new(HashSet::new()),
        }
    }

    /// 尝试获取指定会话的压缩标记：无压缩进行中时占位并返回 true；
    /// 已有并发压缩进行中返回 false（调用方应跳过本次压缩，永不阻断请求）。
    /// 锁中毒等异常降级为允许压缩（宁可极端重复，不阻断）。
    pub fn try_begin_compression(&self, session_id: &str) -> bool {
        self.compressing_sessions
            .lock()
            .map(|mut set| set.insert(session_id.to_string()))
            .unwrap_or(true)
    }

    /// 释放指定会话的压缩标记（压缩结束后必须调用，无论成败）
    pub fn end_compression(&self, session_id: &str) {
        if let Ok(mut set) = self.compressing_sessions.lock() {
            set.remove(session_id);
        }
    }

    /// 尝试获取压缩 RAII 守卫：无压缩进行中时占位并返回 Some(guard)，
    /// guard 在 Drop 时自动释放标记（含 await 期间请求被取消/panic 的
    /// 路径）；已有并发压缩进行中返回 None（调用方应跳过本次压缩）。
    /// 优先于手工 try/end 配对使用，避免标记泄漏导致会话永远无法再压缩。
    pub fn begin_compression(&self, session_id: &str) -> Option<CompressionGuard<'_>> {
        if self.try_begin_compression(session_id) {
            Some(CompressionGuard {
                state: self,
                session_id: session_id.to_string(),
            })
        } else {
            None
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("app.db")
    }

    pub fn salt_path(&self) -> PathBuf {
        self.data_dir.join("salt.hex")
    }

    /// SQLCipher 密钥文件（hex 编码）。存在即表示可启动时自动解锁；
    /// 老库（仅 salt.hex）需先走 /api/auth/migrate 一次性迁移生成。
    pub fn key_file_path(&self) -> PathBuf {
        self.data_dir.join("db.key")
    }
}

/// 压缩标记 RAII 守卫：Drop 时自动释放会话压缩标记。
///
/// 泄漏防护：原手工 try_begin/end 配对在 compress_context await 期间
/// 请求被取消时 end_compression 永不执行，该会话永远无法再压缩；
/// 改用守卫后取消/panic 路径也必经 Drop。
pub struct CompressionGuard<'a> {
    state: &'a AppState,
    session_id: String,
}

impl Drop for CompressionGuard<'_> {
    fn drop(&mut self) {
        self.state.end_compression(&self.session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 压缩竞态标记语义：同会话互斥、跨会话独立、释放后可再次获取
    #[test]
    fn compression_flag_is_per_session_and_reusable() {
        let state = AppState::new(PathBuf::from("/tmp"));
        assert!(state.try_begin_compression("s1"));
        // 同会话并发获取失败（压缩进行中）
        assert!(!state.try_begin_compression("s1"));
        // 不同会话互不影响
        assert!(state.try_begin_compression("s2"));
        state.end_compression("s1");
        // 释放后可再次获取
        assert!(state.try_begin_compression("s1"));
        state.end_compression("s1");
        state.end_compression("s2");
        assert!(state.try_begin_compression("s2"));
    }

    /// RAII 守卫语义：持守期间占位互斥，Drop 后自动释放可再次获取；
    /// 模拟 await 期间请求取消（守卫提前 drop）不泄漏标记
    #[test]
    fn compression_guard_releases_mark_on_drop() {
        let state = AppState::new(PathBuf::from("/tmp"));
        {
            let _guard = state.begin_compression("s1").expect("首次获取成功");
            // 持守期间同会话再次获取失败
            assert!(state.begin_compression("s1").is_none());
            // 跨会话不受影响
            let _other = state.begin_compression("s2").expect("跨会话独立");
        } // _guard 与其他守卫在此 Drop，模拟请求取消提前释放
        // Drop 后标记已释放，可再次获取（不泄漏）
        let guard = state.begin_compression("s1").expect("Drop 后可再次获取");
        drop(guard);
        assert!(state.begin_compression("s1").is_some());
    }
}
