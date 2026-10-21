pub mod desc;
mod handle;
pub mod reporter;
mod unit;

use futures::Stream;
use futures::stream::FusedStream;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tokio::time::{Duration, Instant};
use unit::Unit;

pub use desc::{GDesc, PDesc};
pub use handle::PHandle;
use reporter::*;

/// Byte accounting is accumulated locally and only merged into the task
/// descriptor when a report is due, so polling a chunk costs one shared lock
/// read, one integer addition and one monotonic clock read at most.
#[pin_project]
pub struct ProgressStream<M, U, S, P>
where
  M: Stream<Item = U>,
  U: Unit,
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  #[pin]
  stream: M,
  handle: Arc<RwLock<PHandle<S, P>>>,
  report_interval: Duration,
  next_report: Instant,
  pending: i64,
}

impl<M, U, S, P> ProgressStream<M, U, S, P>
where
  M: Stream<Item = U>,
  U: Unit,
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  pub fn new(stream: M, handle: Arc<RwLock<PHandle<S, P>>>) -> Self {
    let report_interval = handle.read().unwrap().report_interval;
    Self {
      stream,
      handle,
      report_interval,
      // Report the first chunk immediately, like the previous interval
      // implementation did, so the UI switches to "in progress" right away.
      next_report: Instant::now(),
      pending: 0,
    }
  }
}

impl<M, U, S, P> Stream for ProgressStream<M, U, S, P>
where
  M: Stream<Item = U>,
  U: Unit,
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  type Item = U;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let p = self.project();

    {
      let h = p.handle.read().unwrap();
      let state = &h.desc.status;
      if state.is_stopped() {
        drop(h);
        // Persist bytes received since the last report before parking, then
        // store the waker so we can be woken up when resumed.
        let incr = std::mem::take(p.pending);
        let mut h = p.handle.write().unwrap();
        h.store_unreported_progress(incr);
        h.store_waker(cx.waker().clone());
        return Poll::Pending;
      }

      if !state.is_in_progress() {
        return Poll::Ready(None);
      }
    }

    match p.stream.poll_next(cx) {
      Poll::Ready(Some(item)) => {
        *p.pending += item.unit_size();
        if *p.next_report <= Instant::now() {
          let incr = std::mem::take(p.pending);
          let mut h = p.handle.write().unwrap();
          h.add_progress(incr);
          h.flush_progress();
          *p.next_report = Instant::now() + *p.report_interval;
        }
        Poll::Ready(Some(item))
      }
      Poll::Ready(None) => {
        // Fold the bytes received since the last report into the descriptor
        // before completing, so the persisted progress stays exact.
        let incr = std::mem::take(p.pending);
        let mut h = p.handle.write().unwrap();
        h.add_progress(incr);
        h.mark_completed();
        Poll::Ready(None)
      }
      Poll::Pending => Poll::Pending,
    }
  }
}

impl<M, U, S, P> FusedStream for ProgressStream<M, U, S, P>
where
  M: Stream<Item = U>,
  U: Unit,
  S: Sink,
  P: Clone + Serialize + for<'de> Deserialize<'de>,
{
  fn is_terminated(&self) -> bool {
    self.handle.read().unwrap().status().is_terminated()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tasks::streams::desc::PStatus;
  use futures::StreamExt;
  use std::path::PathBuf;
  use std::sync::Mutex;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::time::Instant as StdInstant;
  use tokio_util::bytes::Bytes;

  static ZEROS: [u8; 1024] = [0; 1024];

  #[derive(Clone, Default)]
  struct TestSink {
    reports: Arc<Mutex<Vec<i64>>>,
    completions: Arc<AtomicUsize>,
  }

  impl TestSink {
    fn report_count(&self) -> usize {
      self.reports.lock().unwrap().len()
    }

    fn completion_count(&self) -> usize {
      self.completions.load(Ordering::SeqCst)
    }
  }

  impl Sink for TestSink {
    fn report_progress(
      &self,
      _task_id: u32,
      _task_group: Option<&str>,
      current: i64,
      _total: i64,
      _percentage: u32,
      _estimated_time: Option<f64>,
      _speed: f64,
    ) {
      self.reports.lock().unwrap().push(current);
    }

    fn report_completion(&self, _task_id: u32, _task_group: Option<&str>) {
      self.completions.fetch_add(1, Ordering::SeqCst);
    }

    fn report_stopped(&self, _task_id: u32, _task_group: Option<&str>) {}
    fn report_cancelled(&self, _task_id: u32, _task_group: Option<&str>) {}
    fn report_started(&self, _task_id: u32, _task_group: Option<&str>, _total: i64) {}
    fn report_failed(&self, _task_id: u32, _task_group: Option<&str>, _reason: String) {}
  }

  fn test_handle(
    sink: &TestSink,
    interval: Duration,
    total: i64,
    tag: &str,
  ) -> (Arc<RwLock<PHandle<TestSink, ()>>>, PathBuf) {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "sjmcl-progress-{}-{}-{}.json",
      tag,
      std::process::id(),
      nanos
    ));
    let desc = PDesc::<()>::new(1, None, total, (), PStatus::InProgress);
    let handle = Arc::new(RwLock::new(PHandle::new(
      desc,
      interval,
      path.clone(),
      Reporter::new(total, interval, sink.clone()),
    )));
    (handle, path)
  }

  fn chunk_stream(count: usize) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    futures::stream::iter((0..count).map(|_| Ok(Bytes::from_static(&ZEROS))))
  }

  #[tokio::test]
  async fn progress_events_are_coalesced_instead_of_emitted_per_chunk() {
    // A one hour interval guarantees that only the very first chunk can be
    // reported; the remaining 999 chunks must only update the accumulator.
    let sink = TestSink::default();
    let (handle, path) = test_handle(&sink, Duration::from_secs(3600), 1_024_000, "coalesce");

    let mut stream = Box::pin(ProgressStream::new(chunk_stream(1000), handle.clone()));
    while stream.next().await.is_some() {}

    assert_eq!(
      sink.report_count(),
      1,
      "progress must be reported once, not once per chunk"
    );
    assert_eq!(sink.reports.lock().unwrap()[0], 1024);
    assert_eq!(sink.completion_count(), 1);

    {
      let h = handle.read().unwrap();
      assert_eq!(h.desc.current, 1_024_000);
      assert!(h.desc.status == PStatus::Completed);
    }

    // The final descriptor persisted for resume/restart is exact.
    let saved = PDesc::<()>::load(&path).unwrap();
    assert_eq!(saved.current, 1_024_000);
    assert!(saved.status == PStatus::Completed);

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn progress_events_follow_the_report_interval() {
    let sink = TestSink::default();
    let (handle, path) = test_handle(&sink, Duration::from_millis(100), 10 * 1024, "interval");
    let mut stream = Box::pin(ProgressStream::new(chunk_stream(10), handle.clone()));

    let _ = stream.next().await.unwrap();
    for _ in 0..4 {
      let _ = stream.next().await.unwrap();
    }
    assert_eq!(
      sink.report_count(),
      1,
      "burst within the interval is merged"
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    let _ = stream.next().await.unwrap();
    assert_eq!(
      sink.report_count(),
      2,
      "the next report is emitted once the interval elapsed"
    );
    // The second report carries all bytes received so far, not just the last chunk.
    assert_eq!(*sink.reports.lock().unwrap().last().unwrap(), 6 * 1024);

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn stopping_pauses_polling_and_resuming_keeps_progress() {
    let sink = TestSink::default();
    let (handle, path) = test_handle(&sink, Duration::from_secs(3600), 5 * 1024, "pause");
    let mut stream = Box::pin(ProgressStream::new(chunk_stream(5), handle.clone()));

    assert!(stream.next().await.is_some());
    handle.write().unwrap().mark_stopped();

    let paused = tokio::time::timeout(Duration::from_millis(50), stream.next()).await;
    assert!(paused.is_err(), "a stopped task must not consume chunks");

    handle.write().unwrap().mark_resumed();
    let mut received = 0;
    while tokio::time::timeout(Duration::from_secs(2), stream.next())
      .await
      .unwrap()
      .is_some()
    {
      received += 1;
    }
    assert_eq!(
      received, 4,
      "all remaining chunks are consumed after resume"
    );
    assert_eq!(handle.read().unwrap().desc.current, 5 * 1024);

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn pausing_persists_unreported_bytes_for_resume() {
    let sink = TestSink::default();
    // A long interval means only the first chunk is ever reported, leaving
    // four chunks pending when the task is paused.
    let (handle, path) = test_handle(&sink, Duration::from_secs(3600), 5 * 1024, "pause-save");
    let mut stream = Box::pin(ProgressStream::new(chunk_stream(5), handle.clone()));

    for _ in 0..5 {
      let _ = stream.next().await.unwrap();
    }
    assert_eq!(handle.read().unwrap().desc.current, 1024);

    handle.write().unwrap().mark_stopped();
    let paused = tokio::time::timeout(Duration::from_millis(50), stream.next()).await;
    assert!(paused.is_err());

    // The pause path folds the four unreported chunks into the persisted
    // descriptor, so a restart resumes from the exact byte offset.
    let saved = PDesc::<()>::load(&path).unwrap();
    assert!(saved.status == PStatus::Stopped);
    assert_eq!(saved.current, 5 * 1024);

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn cancellation_stops_consumption_and_emits_no_completion() {
    let sink = TestSink::default();
    let (handle, path) = test_handle(&sink, Duration::from_secs(3600), 5 * 1024, "cancel");
    let mut stream = Box::pin(ProgressStream::new(chunk_stream(5), handle.clone()));

    assert!(stream.next().await.is_some());
    handle.write().unwrap().mark_cancelled();

    // A cancelled task must stop consuming chunks immediately and must not
    // report completion, otherwise a cancelled download would be counted as
    // finished in the UI and in the persisted descriptor.
    assert!(stream.next().await.is_none());
    assert_eq!(sink.completion_count(), 0);
    assert!(handle.read().unwrap().status().is_cancelled());

    let saved = PDesc::<()>::load(&path).unwrap();
    assert!(saved.status == PStatus::Cancelled);

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn per_chunk_accounting_microbench() {
    // Indicative comparison of the old and new per-chunk accounting paths.
    // Run with --nocapture to see the measured numbers.
    const CHUNKS: usize = 200_000;

    let sink = TestSink::default();
    let (handle, path) = test_handle(&sink, Duration::from_secs(1), 0, "bench");

    let legacy_start = StdInstant::now();
    {
      let mut interval = tokio::time::interval(Duration::from_secs(1));
      let waker = futures::task::noop_waker_ref();
      let mut cx = Context::from_waker(waker);
      for _ in 0..CHUNKS {
        let mut h = handle.write().unwrap();
        h.add_progress(1024);
        let _ = interval.poll_tick(&mut cx);
      }
    }
    let legacy = legacy_start.elapsed();

    let current_start = StdInstant::now();
    {
      let mut pending = 0i64;
      let mut next_report = Instant::now();
      for _ in 0..CHUNKS {
        {
          let h = handle.read().unwrap();
          assert!(h.status().is_in_progress());
        }
        pending += 1024;
        if next_report <= Instant::now() {
          let incr = std::mem::take(&mut pending);
          let mut h = handle.write().unwrap();
          h.add_progress(incr);
          next_report = Instant::now() + Duration::from_secs(1);
        }
      }
    }
    let current = current_start.elapsed();

    println!(
      "per-chunk accounting over {CHUNKS} chunks: legacy(write-lock+interval poll) {:?} ({:.0} ns/chunk), current(read-lock+Instant) {:?} ({:.0} ns/chunk)",
      legacy,
      legacy.as_nanos() as f64 / CHUNKS as f64,
      current,
      current.as_nanos() as f64 / CHUNKS as f64
    );

    let _ = std::fs::remove_file(&path);
  }
}
