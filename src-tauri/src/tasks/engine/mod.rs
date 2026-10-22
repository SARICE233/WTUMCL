//! Download engine abstraction.
//!
//! A [`DownloadEngine`] performs the transfer of one artifact from a source
//! that the caller has already selected. Mirror selection, source health,
//! cross-source failover and the retry budget that belongs to each source stay
//! in the task layer (`tasks::download`); the engine owns only the per-source
//! mechanics: sending the request, resuming a partial file, checking the
//! announced size, validating SHA-1 and reporting progress.
//!
//! The trait exists so the transfer strategy can evolve without touching the
//! task system or the failover policy. [`SequentialEngine`] copies a single
//! response body at a time and reproduces the behaviour of the pre-abstraction
//! implementation; a future `SegmentedEngine` can split a file into ranges and
//! fetch them concurrently while still speaking the same
//! [`FetchPlan`]/[`ProgressSink`] contract. Callers depend on the trait rather
//! than on the concrete strategy, which also lets the shadow comparison in
//! `sequential` drive the engine and the legacy production path with identical
//! inputs.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use sjmcl_types::error::SJMCLResult;
use tauri::Url;
use tauri_plugin_http::reqwest::Client;

pub mod sequential;

pub use sequential::SequentialEngine;

/// Everything a single transfer needs to know about the artifact and its
/// source.
///
/// A plan is pure data: it carries no task handle, no configuration and no
/// health state, so the engine can be tested without an `AppHandle` and a
/// future segmented engine can split the same plan across workers. The
/// per-source retry policy that is not part of the artifact itself
/// (the header deadline and the rate limiter) lives on
/// [`SequentialEngine`] because [`DownloadEngine::fetch`] receives no other
/// arguments.
#[derive(Debug, Clone)]
pub struct FetchPlan {
  /// Final path of the artifact. The engine creates missing parent
  /// directories and writes the body here.
  pub dest: PathBuf,
  /// Size announced by the manifest, when the caller knows it. A source whose
  /// announced `Content-Length` differs is rejected before the body is
  /// transferred.
  pub size: Option<u64>,
  /// Expected SHA-1 of the finished file. `None` skips validation.
  pub sha1: Option<String>,
  /// Ordered sources this transfer may use. The sequential engine transfers
  /// from the first entry; the failover driver already picked it and keeps the
  /// remaining candidates for later attempts, so this list normally contains
  /// exactly one URL. It is a list because a segmented engine will be allowed
  /// to spread ranges over several mirrors.
  pub sources: Vec<Url>,
  /// Byte offset already present in `dest`. A non-zero offset is sent as a
  /// `Range` request and must be answered with `206 Partial Content`.
  pub resume_offset: u64,
}

/// Receives the observable effects of a running transfer.
///
/// [`on_bytes`](ProgressSink::on_bytes) is the only required callback: the
/// engine calls it after each chunk reaches the destination file, which is
/// enough for a progress bar or a test that only counts bytes. The remaining
/// callbacks are no-ops by default so a test can plug in
/// [`NullProgressSink`] unchanged; the production task adapter overrides them
/// to keep pause/cancel/task-descriptor semantics identical to the
/// pre-engine implementation.
pub trait ProgressSink: Send + Sync {
  /// Reports bytes that were just written to the destination file.
  fn on_bytes(&mut self, delta: u64);

  /// Reports the announced total length of the response before the body is
  /// copied. `-1` means unknown, matching the task descriptor convention.
  fn on_total(&mut self, _total: i64) {}

  /// Reports that the first response of this transfer was accepted, so the
  /// task can switch from "waiting" to "in progress".
  fn on_started(&mut self) {}

  /// Whether the consumer cancelled the transfer. The engine checks this
  /// before every chunk and removes the partial file when it becomes true.
  fn is_cancelled(&self) -> bool {
    false
  }

  /// Resolves once the transfer may continue. A paused consumer parks the
  /// transfer here instead of buffering the body in memory.
  fn wait_if_paused(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(std::future::ready(()))
  }

  /// Reports that the whole body was written, before SHA-1 validation runs.
  /// The production sink uses it to complete the task descriptor exactly when
  /// the old `ProgressStream` did.
  fn on_complete(&mut self) {}
}

/// A [`ProgressSink`] that ignores every callback.
///
/// Used by tests and by any caller that does not need progress; keeping it in
/// the engine module means a new engine can be exercised without pulling in
/// the task system.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullProgressSink;

impl NullProgressSink {
  pub fn new() -> Self {
    Self
  }
}

impl ProgressSink for NullProgressSink {
  fn on_bytes(&mut self, _delta: u64) {}
}

/// How a successful transfer ended.
///
/// Cancellation is not an error: the failover driver stops trying sources and
/// the task just disappears, so it is reported separately from `Ok(())`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchOutcome {
  /// The body was written and, when a hash was supplied, validated.
  Completed,
  /// The consumer cancelled; any partial file was removed.
  Cancelled,
}

/// Transfers one artifact according to a [`FetchPlan`].
///
/// The implementation receives everything it needs, so the failover driver can
/// swap strategies without changing how sources are chosen or how failures are
/// recorded. The boxed future keeps the trait object-safe: callers hold a
/// `&dyn DownloadEngine` selected once per task instead of branching on a
/// concrete type.
pub trait DownloadEngine: Send + Sync {
  fn fetch<'a>(
    &'a self,
    client: &'a Client,
    plan: &'a FetchPlan,
    sink: &'a mut dyn ProgressSink,
  ) -> Pin<Box<dyn Future<Output = SJMCLResult<FetchOutcome>> + Send + 'a>>;
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::path::PathBuf;

  use crate::tasks::download::DownloadParam;
  use crate::tasks::download::download_with_policy;
  use crate::tasks::download::tests::{FakeHttpServer, FakeReply, sha1_hex, unique_temp_path};

  fn http_client() -> Client {
    Client::builder().no_proxy().build().unwrap()
  }

  fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 251) as u8).collect()
  }

  fn plan(url: &str, dest: PathBuf, size: Option<u64>, sha1: Option<String>) -> FetchPlan {
    FetchPlan {
      dest,
      size,
      sha1,
      sources: vec![Url::parse(url).unwrap()],
      resume_offset: 0,
    }
  }

  /// The engine and the production failover path must produce the same
  /// artifact for the same payload. This is the shadow comparison: the legacy
  /// entry point (`download_with_policy`, unchanged by the extraction) and a
  /// direct `SequentialEngine` call each receive one fake server serving the
  /// same bytes, and the resulting files are compared byte for byte.
  #[tokio::test]
  async fn shadow_comparison_matches_the_legacy_path_byte_for_byte() {
    let data = payload(4096);
    let truth = sha1_hex(&data);
    let engine_server =
      FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose { body: data.clone() }]);
    let legacy_server =
      FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose { body: data.clone() }]);
    let client = http_client();

    let engine_dest = unique_temp_path("shadow-engine");
    let engine_plan = plan(
      &engine_server.url,
      engine_dest.clone(),
      Some(data.len() as u64),
      Some(truth.clone()),
    );
    let mut sink = NullProgressSink::new();
    let outcome = SequentialEngine::new()
      .fetch(&client, &engine_plan, &mut sink)
      .await
      .expect("the engine transfer must succeed");

    let legacy_dest = unique_temp_path("shadow-legacy");
    let param = DownloadParam {
      src: Url::parse(&legacy_server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: Some(data.len() as u64),
    };
    download_with_policy(
      &client,
      &param,
      &[],
      param.size,
      &legacy_dest,
      0,
      None,
      None,
    )
    .await
    .expect("the legacy path must succeed");

    let engine_bytes = std::fs::read(&engine_dest).unwrap();
    let legacy_bytes = std::fs::read(&legacy_dest).unwrap();

    assert_eq!(outcome, FetchOutcome::Completed);
    assert_eq!(
      engine_bytes, legacy_bytes,
      "engine and legacy path must write identical bytes"
    );
    assert_eq!(engine_bytes, data);
    assert_eq!(sha1_hex(&engine_bytes), sha1_hex(&legacy_bytes));
    assert_eq!(sha1_hex(&engine_bytes), truth);

    let _ = std::fs::remove_file(&engine_dest);
    let _ = std::fs::remove_file(&legacy_dest);
  }

  /// A failing plan must fail both implementations the same way: an error,
  /// and no artifact left behind.
  #[tokio::test]
  async fn shadow_comparison_matches_the_legacy_failure_outcome() {
    // `send_with_retry_budget` makes up to three header attempts, so each fake
    // server must answer all of them.
    let engine_server = FakeHttpServer::spawn(vec![
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
    ]);
    let legacy_server = FakeHttpServer::spawn(vec![
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
      FakeReply::Status {
        code: 404,
        reason: "Not Found",
      },
    ]);
    let client = http_client();

    let engine_dest = unique_temp_path("shadow-fail-engine");
    let engine_plan = plan(&engine_server.url, engine_dest.clone(), None, None);
    let mut sink = NullProgressSink::new();
    let engine_error = SequentialEngine::new()
      .fetch(&client, &engine_plan, &mut sink)
      .await
      .expect_err("a 404 source must fail the transfer");

    let legacy_dest = unique_temp_path("shadow-fail-legacy");
    let param = DownloadParam {
      src: Url::parse(&legacy_server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: None,
      size: None,
    };
    let legacy_error =
      download_with_policy(&client, &param, &[], None, &legacy_dest, 0, None, None)
        .await
        .expect_err("the legacy path must fail as well");

    assert!(engine_error.0.contains("404"), "{}", engine_error.0);
    assert!(legacy_error.0.contains("404"), "{}", legacy_error.0);
    assert!(!engine_dest.exists());
    assert!(!legacy_dest.exists());
  }

  struct CountingSink {
    bytes: u64,
    chunks: u64,
    totals: Vec<i64>,
    started: u64,
    completed: u64,
  }

  impl CountingSink {
    fn new() -> Self {
      Self {
        bytes: 0,
        chunks: 0,
        totals: Vec::new(),
        started: 0,
        completed: 0,
      }
    }
  }

  impl ProgressSink for CountingSink {
    fn on_bytes(&mut self, delta: u64) {
      self.bytes += delta;
      self.chunks += 1;
    }

    fn on_total(&mut self, total: i64) {
      self.totals.push(total);
    }

    fn on_started(&mut self) {
      self.started += 1;
    }

    fn on_complete(&mut self) {
      self.completed += 1;
    }
  }

  #[tokio::test]
  async fn engine_reports_every_byte_through_the_progress_sink() {
    let data = payload(8192);
    // Announce the length so the sink also observes the total.
    let server = FakeHttpServer::spawn(vec![FakeReply::TruncatedBody {
      body: data.clone(),
      sent: data.len(),
    }]);
    let dest = unique_temp_path("engine-progress");
    let plan = plan(
      &server.url,
      dest.clone(),
      Some(data.len() as u64),
      Some(sha1_hex(&data)),
    );
    let mut sink = CountingSink::new();

    let outcome = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect("transfer must succeed");

    assert_eq!(outcome, FetchOutcome::Completed);
    assert_eq!(sink.bytes, data.len() as u64);
    assert!(sink.chunks >= 1);
    assert_eq!(sink.totals, vec![data.len() as i64]);
    assert_eq!(sink.started, 1);
    assert_eq!(sink.completed, 1);
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    let _ = std::fs::remove_file(&dest);
  }

  struct CancelAfterFirstChunk {
    bytes: u64,
  }

  impl ProgressSink for CancelAfterFirstChunk {
    fn on_bytes(&mut self, delta: u64) {
      self.bytes += delta;
    }

    fn is_cancelled(&self) -> bool {
      self.bytes > 0
    }
  }

  #[tokio::test]
  async fn engine_stops_on_cancellation_and_removes_the_partial_file() {
    let data = payload(8192);
    let server = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose { body: data.clone() }]);
    let dest = unique_temp_path("engine-cancel");
    let plan = plan(&server.url, dest.clone(), None, Some(sha1_hex(&data)));
    let mut sink = CancelAfterFirstChunk { bytes: 0 };

    let outcome = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect("cancellation is not an error");

    assert_eq!(outcome, FetchOutcome::Cancelled);
    assert!(
      !dest.exists(),
      "a cancelled transfer must not leave bytes behind"
    );
    assert!(sink.bytes > 0);
  }

  #[tokio::test]
  async fn engine_rejects_a_plan_without_sources() {
    let dest = unique_temp_path("engine-no-source");
    let plan = FetchPlan {
      dest: dest.clone(),
      size: None,
      sha1: None,
      sources: Vec::new(),
      resume_offset: 0,
    };
    let mut sink = NullProgressSink::new();

    let error = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect_err("an empty source list must be rejected");

    assert!(
      error.0.contains("contains no source URL"),
      "unexpected error: {}",
      error.0
    );
    assert!(!dest.exists());
  }

  #[tokio::test]
  async fn engine_requires_206_when_resuming() {
    let data = payload(4096);
    // The server answers the Range request with the whole file and 200 OK.
    let server = FakeHttpServer::spawn(vec![FakeReply::TruncatedBody {
      body: data.clone(),
      sent: data.len(),
    }]);
    let dest = unique_temp_path("engine-resume-200");
    let prefix = &data[..2048];
    std::fs::write(&dest, prefix).unwrap();
    let plan = FetchPlan {
      dest: dest.clone(),
      size: None,
      sha1: None,
      sources: vec![Url::parse(&server.url).unwrap()],
      resume_offset: 2048,
    };
    let mut sink = NullProgressSink::new();

    let error = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect_err("a 200 response to a Range request must be rejected");

    assert!(
      error.0.contains("expected 206"),
      "unexpected error: {}",
      error.0
    );
    assert_eq!(
      std::fs::read(&dest).unwrap(),
      prefix,
      "no bytes may be appended after a rejected resume"
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn engine_resumes_from_the_persisted_offset_on_206() {
    let data = payload(4096);
    let server = FakeHttpServer::spawn(vec![FakeReply::RangeTail {
      body: data.clone(),
      offset: 2048,
    }]);
    let dest = unique_temp_path("engine-resume-206");
    std::fs::write(&dest, &data[..2048]).unwrap();
    let plan = FetchPlan {
      dest: dest.clone(),
      size: Some(data.len() as u64),
      sha1: Some(sha1_hex(&data)),
      sources: vec![Url::parse(&server.url).unwrap()],
      resume_offset: 2048,
    };
    let mut sink = NullProgressSink::new();

    let outcome = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect("a 206 tail must complete the file");

    assert_eq!(outcome, FetchOutcome::Completed);
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    let heads = server.request_heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 1);
    assert!(
      heads[0].to_ascii_lowercase().contains("range: bytes=2048-"),
      "the engine must send the persisted offset as a Range header, got: {}",
      heads[0]
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn engine_rejects_a_size_mismatch_before_writing() {
    let server = FakeHttpServer::spawn(vec![FakeReply::TruncatedBody {
      body: vec![0xee; 8192],
      sent: 8192,
    }]);
    let dest = unique_temp_path("engine-size-mismatch");
    let plan = plan(&server.url, dest.clone(), Some(4096), None);
    let mut sink = NullProgressSink::new();

    let error = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect_err("a wrong Content-Length must be rejected");

    assert!(
      error
        .0
        .contains("Size mismatch: expected 4096 bytes, the source announced 8192"),
      "unexpected error: {}",
      error.0
    );
    assert!(
      !dest.exists(),
      "no body byte may be written before the precheck"
    );
  }

  #[tokio::test]
  async fn engine_retries_sha1_on_the_same_source_when_allowed() {
    let data = payload(4096);
    let truth = sha1_hex(&data);
    let server = FakeHttpServer::spawn(vec![
      FakeReply::BodyEndedByClose {
        body: data[..512].to_vec(),
      },
      FakeReply::BodyEndedByClose { body: data.clone() },
    ]);
    let dest = unique_temp_path("engine-sha1-retry");
    let plan = plan(&server.url, dest.clone(), None, Some(truth.clone()));
    let mut sink = NullProgressSink::new();

    let outcome = SequentialEngine::new()
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect("the truncated first body must be retried and succeed");

    assert_eq!(outcome, FetchOutcome::Completed);
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert_eq!(sha1_hex(&std::fs::read(&dest).unwrap()), truth);
    let heads = server.request_heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 2);
    assert!(
      !heads[1].to_ascii_lowercase().contains("range:"),
      "the SHA-1 retry must restart from scratch, not resume the truncated file"
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn engine_returns_the_sha1_error_and_removes_the_file_when_retry_is_disallowed() {
    let data = payload(1024);
    let mut corrupted = data.clone();
    corrupted[0] ^= 0xff;
    let server = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose { body: corrupted }]);
    let dest = unique_temp_path("engine-sha1-fail");
    let plan = plan(&server.url, dest.clone(), None, Some(sha1_hex(&data)));
    let mut sink = NullProgressSink::new();

    let error = SequentialEngine::new()
      .with_sha1_retry_on_same_source(false)
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect_err("a corrupted body must fail when the driver can try another source");

    assert!(
      error.0.contains("SHA1 validation failed"),
      "unexpected error: {}",
      error.0
    );
    assert!(!dest.exists(), "an unverified artifact must be deleted");
    assert_eq!(server.request_heads.lock().unwrap().len(), 1);
  }

  #[tokio::test]
  async fn engine_applies_the_rate_limiter() {
    let data = payload(4096);
    let server = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose { body: data.clone() }]);
    let dest = unique_temp_path("engine-limiter");
    let plan = plan(&server.url, dest.clone(), None, Some(sha1_hex(&data)));
    let mut sink = NullProgressSink::new();

    let outcome = SequentialEngine::new()
      .with_limiter(Some(async_speed_limit::Limiter::new(1024.0 * 1024.0)))
      .fetch(&http_client(), &plan, &mut sink)
      .await
      .expect("a limited transfer must still complete");

    assert_eq!(outcome, FetchOutcome::Completed);
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    let _ = std::fs::remove_file(&dest);
  }
}
