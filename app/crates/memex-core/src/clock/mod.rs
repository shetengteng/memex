//! 抽象的"现在"——把所有 `chrono::Utc::now()` 调用绕到 trait 后面，
//! 让需要确定性时间的单元测试可以注入 [`FrozenClock`] 拿到固定时间戳。
//!
//! ## 设计
//!
//! - trait 只 expose [`Clock::now_utc`]。需要本地时间分桶的代码（如 daily
//!   聚合）自己 `with_timezone(&Local)` 转换；这样 Frozen 注入下 UTC 部分
//!   仍然确定，Local 仍受 OS 时区影响——daily 测试本来就在固定时区下跑。
//! - `Send + Sync + Debug`：业务代码常把 `Db`（含 clock）跨线程共享，
//!   且 tracing 偶尔需要 `Debug` 出当前 clock 类型（区分 system vs frozen）。
//!
//! ## 用法
//!
//! ```no_run
//! use std::sync::Arc;
//! use memex_core::clock::{Clock, FrozenClock};
//! use memex_core::storage::db::Db;
//!
//! // 测试中注入固定时间
//! let clock = Arc::new(FrozenClock::epoch_2026());
//! let db = Db::open_in_memory_with_clock(clock.clone()).unwrap();
//! // 之后所有 db 内部对 clock.now_utc() 的访问都返回 2026-01-01T00:00:00Z
//! # let _ = db;
//! ```

use std::fmt::Debug;
use std::sync::Arc;

use chrono::{DateTime, Utc};

/// 通用"现在"接口。生产代码默认走 [`SystemClock`]；测试覆盖 deterministic
/// 时间戳的代码点用 [`FrozenClock`]。
pub trait Clock: Send + Sync + Debug {
    fn now_utc(&self) -> DateTime<Utc>;
}

/// 真实系统时钟。直接转发到 `chrono::Utc::now()`。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// 固定时间戳 clock，用于单元测试。
#[derive(Debug, Clone)]
pub struct FrozenClock(DateTime<Utc>);

impl FrozenClock {
    pub fn new(t: DateTime<Utc>) -> Self {
        Self(t)
    }

    /// 一个未来的明确锚点（`2026-01-01T00:00:00Z`），避免与真实"今天"巧合
    /// 让 deterministic 测试结果意外通过。
    pub fn epoch_2026() -> Self {
        let naive = chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
            .expect("INVARIANT: 2026-01-01 is a valid date")
            .and_hms_opt(0, 0, 0)
            .expect("INVARIANT: 00:00:00 is a valid time");
        Self(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    }
}

impl Clock for FrozenClock {
    fn now_utc(&self) -> DateTime<Utc> {
        self.0
    }
}

/// `Db` / 跨线程上下文常用的 clock 别名。
pub type ArcClock = Arc<dyn Clock>;

/// 便利构造：`SystemClock` 包装为 [`ArcClock`]。
pub fn system() -> ArcClock {
    Arc::new(SystemClock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_returns_constant_value() {
        let c = FrozenClock::epoch_2026();
        let t1 = c.now_utc();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let t2 = c.now_utc();
        assert_eq!(t1, t2);
        assert_eq!(t1.to_rfc3339(), "2026-01-01T00:00:00+00:00");
    }

    #[test]
    fn system_advances() {
        let c = SystemClock;
        let t1 = c.now_utc();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let t2 = c.now_utc();
        assert!(t2 > t1, "system clock should advance");
    }

    #[test]
    fn frozen_with_explicit_timestamp() {
        let anchor = chrono::DateTime::parse_from_rfc3339("2030-06-15T12:00:00+00:00")
            .unwrap()
            .with_timezone(&Utc);
        let c = FrozenClock::new(anchor);
        assert_eq!(c.now_utc(), anchor);
    }
}
