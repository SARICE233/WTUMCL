use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::task::Waker;
use tokio::time::Duration;

use crate::tasks::streams::desc::{PDesc, PStatus};
use crate::tasks::streams::reporter::{Reporter, Sink};

pub struct PHandle<S, P>
where
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  pub desc: PDesc<P>,
  pub path: PathBuf,
  pub reporter: Reporter<S>,
  /// Minimum interval between two persisted / emitted progress reports.
  ///
  /// Progress bytes are accumulated per chunk and only flushed when this
  /// interval has elapsed, so the download hot loop never serializes JSON or
  /// emits an IPC event for every chunk.
  pub report_interval: Duration,
  pub waker: Option<Waker>,
}

impl<S, P> PHandle<S, P>
where
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  pub fn new(
    desc: PDesc<P>,
    report_interval: Duration,
    path: PathBuf,
    reporter: Reporter<S>,
  ) -> Self {
    Self {
      desc,
      path,
      reporter,
      report_interval,
      waker: None,
    }
  }

  pub fn mark_stopped(&mut self) {
    self.desc.stop();
    self.desc.save(&self.path).unwrap();
    self
      .reporter
      .report_stopped(self.desc.task_id, self.desc.task_group.as_deref());
  }

  pub fn mark_resumed(&mut self) {
    self.desc.resume();
    self.desc.save(&self.path).unwrap();
    self.reporter.report_started(
      self.desc.task_id,
      self.desc.task_group.as_deref(),
      self.desc.total,
    );

    // Wake up any waiting poll_next calls
    if let Some(waker) = self.waker.take() {
      waker.wake();
    }
  }

  pub fn mark_cancelled(&mut self) {
    self.desc.cancel();
    self.desc.save(&self.path).unwrap();
    self
      .reporter
      .report_cancelled(self.desc.task_id, self.desc.task_group.as_deref());
  }

  pub fn mark_completed(&mut self) {
    self.desc.complete();
    self.desc.save(&self.path).unwrap();
    self
      .reporter
      .report_completion(self.desc.task_id, self.desc.task_group.as_deref());
  }

  pub fn mark_started(&mut self) {
    self.desc.start();
    self.desc.save(&self.path).unwrap();
    self.reporter.report_started(
      self.desc.task_id,
      self.desc.task_group.as_deref(),
      self.desc.total,
    );
  }

  pub fn mark_failed(&mut self, reason: String) {
    self.desc.fail();
    self.desc.save(&self.path).unwrap();
    self
      .reporter
      .report_failed(self.desc.task_id, self.desc.task_group.as_deref(), reason);
  }

  pub fn status(&self) -> &PStatus {
    &self.desc.status
  }

  pub fn store_waker(&mut self, waker: Waker) {
    self.waker = Some(waker);
  }

  pub fn set_total(&mut self, total: i64) {
    if total > self.desc.total {
      self.desc.total = total;
      self.desc.save(&self.path).unwrap();
      self.reporter.set_total(total);
    }
  }

  /// Accumulates already received bytes to be reported later.
  ///
  /// Unlike flush_progress, this neither writes the task descriptor to disk
  /// nor emits an event, so it is cheap enough to be called for every
  /// downloaded chunk.
  pub fn add_progress(&mut self, incr: i64) {
    self.desc.increment_progress(incr);
  }

  /// Records bytes that were already written to the destination file but not
  /// reported yet, even though the task is no longer in progress.
  ///
  /// This is used when a pause is observed, so a later resume or restart starts
  /// from the exact byte offset instead of up to one report interval earlier.
  pub fn store_unreported_progress(&mut self, incr: i64) {
    if incr <= 0 {
      return;
    }
    self.desc.current += incr;
    self.desc.save(&self.path).unwrap();
  }

  /// Persists the accumulated progress and emits exactly one progress event.
  pub fn flush_progress(&mut self) {
    self.desc.save(&self.path).unwrap();
    self.reporter.report_progress(
      self.desc.task_id,
      self.desc.task_group.as_deref(),
      self.desc.current,
    );
  }
}
