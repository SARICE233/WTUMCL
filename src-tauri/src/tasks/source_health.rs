use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use url::Url;

/// Consecutive failures after which a host is skipped for `DISABLE_TTL`.
const DISABLE_AFTER_FAILURES: u32 = 3;

/// How long a disabled host stays out of rotation. Long enough to stop a dying
/// mirror from costing every task a timeout, short enough to recover on its own.
const DISABLE_TTL: Duration = Duration::from_secs(300);

/// Bounds for the per-source response-header timeout, mirroring PCL's adaptive
/// `clamp(2 * measured, 15s, 30s)` policy (`ModNet.vb:189-191`).
const MIN_SOURCE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_SOURCE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default)]
struct HostState {
  /// Exponentially weighted mean of the last successful response-header
  /// latency, in milliseconds. `None` until the host answers at least once.
  ewma_ms: Option<f64>,
  consecutive_failures: u32,
  disabled_until: Option<Instant>,
}

/// Process-wide, per-host download health.
///
/// Kept deliberately small: the point is to stop picking a source that has just
/// proven itself dead, not to build a scheduler. Two launchers that inspired this
/// (PCL `Source.FailCount`, PCL CE `DownloadSource.IsFailed`) track the same two
/// numbers per source.
pub struct SourceHealth {
  hosts: Mutex<HashMap<String, HostState>>,
}

impl SourceHealth {
  fn new() -> Self {
    Self {
      hosts: Mutex::new(HashMap::new()),
    }
  }

  /// Stable key for a download URL: scheme + host + port.
  pub fn host_key(url: &Url) -> String {
    match (url.host_str(), url.port()) {
      (Some(host), Some(port)) => format!("{host}:{port}"),
      (Some(host), None) => host.to_string(),
      (None, _) => url.as_str().to_string(),
    }
  }

  pub fn record_success(&self, key: &str, latency: Duration) {
    let Some(mut hosts) = self.hosts.lock().ok() else {
      return;
    };
    let state = hosts.entry(key.to_string()).or_default();
    let sample = latency.as_millis() as f64;
    state.ewma_ms = Some(match state.ewma_ms {
      Some(previous) => previous * 0.7 + sample * 0.3,
      None => sample,
    });
    state.consecutive_failures = 0;
    state.disabled_until = None;
  }

  /// Records a failure and returns how many consecutive ones the host has now.
  pub fn record_failure(&self, key: &str) -> u32 {
    let Some(mut hosts) = self.hosts.lock().ok() else {
      return 0;
    };
    let state = hosts.entry(key.to_string()).or_default();
    state.consecutive_failures += 1;
    if state.consecutive_failures >= DISABLE_AFTER_FAILURES {
      state.disabled_until = Some(Instant::now() + DISABLE_TTL);
    }
    state.consecutive_failures
  }

  /// Whether a host should be skipped right now. Expired entries re-arm
  /// themselves so a recovered mirror is tried again instead of staying banned
  /// for the rest of the session.
  pub fn is_disabled(&self, key: &str) -> bool {
    let Ok(mut hosts) = self.hosts.lock() else {
      return false;
    };
    let Some(state) = hosts.get_mut(key) else {
      return false;
    };
    match state.disabled_until {
      Some(until) if Instant::now() >= until => {
        state.disabled_until = None;
        state.consecutive_failures = 0;
        false
      }
      Some(_) => true,
      None => false,
    }
  }

  /// Response-header timeout for a host, derived from its measured latency.
  pub fn timeout_for(&self, key: &str) -> Duration {
    let Ok(hosts) = self.hosts.lock() else {
      return MAX_SOURCE_TIMEOUT;
    };
    let Some(state) = hosts.get(key) else {
      return MAX_SOURCE_TIMEOUT;
    };
    let Some(ewma) = state.ewma_ms else {
      return MAX_SOURCE_TIMEOUT;
    };
    let doubled = Duration::from_millis((ewma * 2.0) as u64);
    doubled.clamp(MIN_SOURCE_TIMEOUT, MAX_SOURCE_TIMEOUT)
  }

  #[cfg(test)]
  fn clear(&self) {
    if let Ok(mut hosts) = self.hosts.lock() {
      hosts.clear();
    }
  }
}

static HEALTH: OnceLock<SourceHealth> = OnceLock::new();

pub fn source_health() -> &'static SourceHealth {
  HEALTH.get_or_init(SourceHealth::new)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fresh() -> SourceHealth {
    SourceHealth::new()
  }

  #[test]
  fn unknown_host_is_enabled_with_the_upper_bound() {
    let health = fresh();
    assert!(!health.is_disabled("a.example"));
    assert_eq!(health.timeout_for("a.example"), MAX_SOURCE_TIMEOUT);
  }

  #[test]
  fn three_consecutive_failures_disable_a_host() {
    let health = fresh();
    assert_eq!(health.record_failure("a.example"), 1);
    assert!(!health.is_disabled("a.example"));
    assert_eq!(health.record_failure("a.example"), 2);
    assert!(!health.is_disabled("a.example"));
    assert_eq!(health.record_failure("a.example"), 3);
    assert!(health.is_disabled("a.example"));
  }

  #[test]
  fn a_success_re_enables_the_host_and_clears_failures() {
    let health = fresh();
    for _ in 0..3 {
      health.record_failure("a.example");
    }
    assert!(health.is_disabled("a.example"));
    health.record_success("a.example", Duration::from_millis(200));
    assert!(!health.is_disabled("a.example"));
    assert_eq!(health.record_failure("a.example"), 1);
  }

  #[test]
  fn timeout_is_twice_the_measured_latency_within_bounds() {
    let health = fresh();
    health.record_success("slow.example", Duration::from_secs(1));
    // 1s measured -> 2s doubled -> clamped up to the 15s floor.
    assert_eq!(health.timeout_for("slow.example"), MIN_SOURCE_TIMEOUT);

    health.record_success("slower.example", Duration::from_secs(9));
    // 9s measured -> 18s doubled -> inside the window.
    assert_eq!(
      health.timeout_for("slower.example"),
      Duration::from_secs(18)
    );

    health.record_success("very-slow.example", Duration::from_secs(40));
    // 40s measured -> 80s doubled -> clamped down to the 30s ceiling.
    assert_eq!(health.timeout_for("very-slow.example"), MAX_SOURCE_TIMEOUT);
  }

  #[test]
  fn host_key_keeps_the_port_when_present() {
    let plain = Url::parse("https://a.example/x").unwrap();
    assert_eq!(SourceHealth::host_key(&plain), "a.example");
    let ported = Url::parse("https://b.example:8443/x").unwrap();
    assert_eq!(SourceHealth::host_key(&ported), "b.example:8443");
  }

  #[test]
  fn hosts_are_independent() {
    let health = fresh();
    health.clear();
    for _ in 0..3 {
      health.record_failure("dead.example");
    }
    assert!(health.is_disabled("dead.example"));
    assert!(!health.is_disabled("alive.example"));
  }
}
