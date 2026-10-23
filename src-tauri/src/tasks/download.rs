use async_speed_limit::Limiter;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_http::reqwest::header::{HeaderMap, HeaderValue};
use tauri_plugin_http::reqwest::{Client, ClientBuilder, Proxy};

use crate::launcher_config::models::{LauncherConfig, ProxyType};
use crate::resource::helpers::source::candidates;
use crate::tasks::engine::{
  DownloadEngine, FetchOutcome, FetchPlan, NullProgressSink, ProgressSink, SequentialEngine,
};
use crate::tasks::source_health::{SourceHealth, source_health};
use crate::tasks::streams::desc::{PDesc, PStatus};
use crate::tasks::streams::reporter::Reporter;
use crate::tasks::*;

// Transfer-level helpers now live in `tasks::engine::sequential`; the tests
// below still exercise them through their historical names.
#[cfg(test)]
use crate::tasks::engine::sequential::{
  DOWNLOAD_BUFFER_SIZE, DOWNLOAD_SHA1_RETRIES, DownloadFailure, DownloadFailureClass,
  DownloadRequestError, classify_download_failure, copy_to_file, ensure_resumed_response,
  into_byte_stream, prepare_dest_file, send_with_retry_budget, validate_dest_sha1,
};
#[cfg(test)]
use futures::StreamExt;
#[cfg(test)]
use tauri_plugin_http::reqwest::StatusCode;
#[cfg(test)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Timeout for establishing the TCP/TLS connection of a download.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum time to wait for the next body chunk before treating the transfer as
/// stalled. Unlike a total deadline this resets on every successful read, so a
/// huge file may take as long as it needs while a dead socket fails fast
/// (PCL2-CE uses 15 s for the same purpose).
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How long an idle connection stays in the download pool.
///
/// Consecutive assets/mod files usually come from the same CDN; keeping the
/// connection for 5 minutes means the next file reuses the existing TCP+TLS
/// session instead of paying another DNS + handshake round trip. PCL2 keeps one
/// long-lived `HttpClient` for its whole download engine for the same reason.
const DOWNLOAD_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Upper bound of idle keep-alive connections kept per host. The automatic
/// concurrency policy opens at most 32 downloads at once, so 64 slots cover a
/// full burst plus the short metadata requests that share the same host.
const DOWNLOAD_MAX_IDLE_PER_HOST: usize = 64;

/// Builds the dedicated HTTP client used for file downloads.
///
/// Why not the shared application client: `utils::web::build_sjmcl_client`
/// sets `ClientBuilder::timeout(10s)`, and reqwest treats that as a *total*
/// deadline that also covers the response body. Any transfer slower than 10 s
/// was therefore cut off mid-file (the truncated body used to be swallowed as
/// an empty chunk, and with a SHA1 the task then failed). Downloads need a
/// per-read timeout and no total deadline, so they get their own client.
///
/// All `DownloadTask`s share the client created by `download_client`, hence a
/// single connection pool with keep-alive and TLS session reuse; the client is
/// never rebuilt per file.
fn download_client_builder() -> ClientBuilder {
  ClientBuilder::new()
    .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
    .read_timeout(DOWNLOAD_READ_TIMEOUT)
    .tcp_keepalive(Duration::from_secs(30))
    .tcp_nodelay(true)
    .pool_idle_timeout(DOWNLOAD_POOL_IDLE_TIMEOUT)
    .pool_max_idle_per_host(DOWNLOAD_MAX_IDLE_PER_HOST)
}

/// Applies the launcher's User-Agent and proxy settings to the download client.
///
/// PCL2 and PCL2-CE both keep one download client for the whole process; the
/// values are read once here, exactly like the shared client built at startup,
/// so changing proxy settings requires a launcher restart.
fn build_download_client(user_agent: Option<&str>, proxy_url: Option<&str>) -> Client {
  let mut builder = download_client_builder();

  if let Some(user_agent) = user_agent
    && let Ok(value) = user_agent.parse::<HeaderValue>()
  {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", value);
    builder = builder.default_headers(headers);
  }

  if let Some(proxy_url) = proxy_url
    && let Ok(proxy) = Proxy::all(proxy_url)
  {
    builder = builder.proxy(proxy);
  }

  builder.build().unwrap_or_else(|error| {
    log::warn!("Failed to build the download HTTP client, falling back: {error}");
    Client::new()
  })
}

static DOWNLOAD_CLIENT: OnceLock<Client> = OnceLock::new();

/// Returns the process-wide download client, creating it on first use.
fn download_client(app_handle: &AppHandle) -> &'static Client {
  DOWNLOAD_CLIENT.get_or_init(|| {
    let config = app_handle.state::<Mutex<LauncherConfig>>();
    let Ok(config) = config.lock() else {
      return build_download_client(None, None);
    };

    let user_agent = format!("SJMCL/{}", config.basic_info.launcher_version);
    let proxy_url = if config.download.proxy.enabled {
      let proxy = &config.download.proxy;
      Some(match proxy.selected_type {
        ProxyType::Http => format!("http://{}:{}", proxy.host, proxy.port),
        ProxyType::Socks => format!("socks5h://{}:{}", proxy.host, proxy.port),
      })
    } else {
      None
    };

    build_download_client(Some(&user_agent), proxy_url.as_deref())
  })
}

/// Converts the persisted byte offset into a safe position in the destination
/// file. Corrupted/legacy descriptors can store a negative `current`; treating
/// it as 0 restarts the file instead of seeking to a huge unsigned offset.
fn resume_offset(current: i64) -> u64 {
  current.max(0) as u64
}

/// Clamps a resume offset against the announced size. A leftover that is not
/// strictly shorter than the target file cannot be its prefix, so it is dropped
/// and the transfer restarts from zero instead of splicing unrelated bytes onto
/// the end of a wrong file.
fn safe_resume_offset(current: i64, expected_size: Option<u64>) -> u64 {
  let offset = resume_offset(current);
  match expected_size {
    Some(size) if offset >= size => 0,
    _ => offset,
  }
}

/// Cancellation can land before the engine ever opened the destination, leaving
/// an empty leftover from an older run in place. An empty artifact is worse than
/// a missing one for existence-based consumers, so the driver drops it as well;
/// anything with real bytes is kept, because it is the base of the next resume.
async fn discard_empty_leftover(path: &Path) {
  if let Ok(metadata) = tokio::fs::metadata(path).await
    && metadata.len() == 0
  {
    let _ = tokio::fs::remove_file(path).await;
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadParam {
  pub src: Url,
  pub dest: PathBuf,
  pub filename: Option<String>,
  pub sha1: Option<String>,
  // Size announced by the manifest, when the caller knows it. Sources whose
  // announced Content-Length differs are rejected before the body is fetched.
  // Older persisted task descriptors lack the field, hence the serde default.
  #[serde(default)]
  pub size: Option<u64>,
}

pub struct DownloadTask {
  p_handle: PTaskHandle,
  param: DownloadParam,
  dest_path: PathBuf,
  // Sources to try after `param.src`, derived from the configured mirrors when
  // the task is created (and again when a descriptor is restored, so a config
  // change also applies to resumed tasks). Never persisted: it is a function of
  // the URL and the launcher configuration.
  alt_srcs: Vec<Url>,
  // Size announced by the manifest, when the caller knows it. A source that
  // announces a different length is rejected before the body is transferred.
  expected_size: Option<u64>,
  // Kept so the existing constructors keep their signature; the reporter
  // currently uses a fixed interval instead of this value.
  #[allow(dead_code)]
  report_interval: Duration,
}

/// Reads only the progress report interval from the launcher config instead of
/// cloning the whole config for every scheduled download task.
fn download_cache_dir(app_handle: &AppHandle) -> PathBuf {
  app_handle
    .state::<Mutex<LauncherConfig>>()
    .lock()
    .unwrap()
    .download
    .cache
    .directory
    .clone()
}

/// Reads the launcher config and asks the source layer for alternatives to
/// `src` (the configured mirrors, in priority order).
fn derive_alt_sources(app_handle: &AppHandle, src: &Url) -> Vec<Url> {
  let config = app_handle.state::<Mutex<LauncherConfig>>();
  let Ok(config) = config.lock() else {
    return Vec::new();
  };
  let mut resolved = candidates(src, &[], &config);
  // `candidates` always returns the input URL first; it is already the task's
  // primary source, so only the tail becomes the alternative list.
  if resolved.first().is_some_and(|first| first == src) {
    resolved.remove(0);
  }
  resolved
}

/// Ordered sources for one task: the param's own URL first, then the candidates
/// derived from the configured mirrors.
fn candidates_for(param: &DownloadParam, alt_srcs: &[Url]) -> Vec<Url> {
  let mut sources: Vec<Url> = Vec::with_capacity(1 + alt_srcs.len());
  for candidate in std::iter::once(&param.src).chain(alt_srcs.iter()) {
    if !sources.iter().any(|existing| existing == candidate) {
      sources.push(candidate.clone());
    }
  }
  sources
}

/// Downloads one artifact, trying every candidate source in order.
///
/// The first source keeps the caller's resume offset; later ones restart from
/// zero because a different mirror may not serve the same byte ranges. Each
/// outcome feeds `SourceHealth`, so a mirror that has just failed three times in
/// a row is skipped by the next task instead of costing it another timeout. The
/// gate is advisory, not absolute: when every candidate is currently disabled it
/// is dropped and the sources are tried anyway (fail-open), because a mirror that
/// recovered can only prove itself by answering a request, and a blanket skip
/// turns a transient outage into a permanently failing task. The per-source
/// transfer itself is delegated to [`DownloadEngine`]; this function only owns
/// the failover policy.
pub(crate) async fn download_with_policy(
  client: &Client,
  param: &DownloadParam,
  alt_srcs: &[Url],
  expected_size: Option<u64>,
  dest_path: &Path,
  initial_offset: u64,
  limiter: Option<Limiter>,
  task_handle: Option<Arc<RwLock<PTaskHandle>>>,
) -> SJMCLResult<()> {
  let sources = candidates_for(param, alt_srcs);
  // A single source keeps the historical "retry the same URL" behaviour, which
  // is what rescues truncation on an otherwise healthy official source.
  let retry_same_source_on_sha1 = sources.len() == 1;
  let mut attempts: Vec<String> = Vec::new();
  let mut last_error: Option<String> = None;
  let mut attempted = 0usize;

  // Read the health gate once, before the first attempt, so the decision cannot
  // drift between sources while a concurrent task updates the same host.
  let disabled: Vec<bool> = sources
    .iter()
    .map(|source| source_health().is_disabled(&SourceHealth::host_key(source)))
    .collect();
  // Fail-open: when every candidate sits inside its penalty window, skip
  // nothing. Failing without sending a single request would keep the task
  // broken for the whole TTL even if the mirror is already back, and the mirror
  // can only ever prove itself by answering a request.
  let fail_open = !sources.is_empty() && disabled.iter().all(|disabled| *disabled);
  if fail_open {
    log::warn!(
      "All {} candidate source(s) for {} are marked unhealthy; trying them anyway as a last resort",
      sources.len(),
      param.src
    );
  }

  for (index, source) in sources.iter().enumerate() {
    let host = SourceHealth::host_key(source);
    if disabled[index] && !fail_open {
      attempts.push(format!("{source} (skipped: marked unhealthy)"));
      continue;
    }

    let offset = if index == 0 { initial_offset } else { 0 };
    let started = Instant::now();
    let headers_timeout = source_health().timeout_for(&host);
    attempted += 1;

    // The engine receives one source per call: source order and health are the
    // driver's responsibility, the transfer contract is the engine's.
    let plan = FetchPlan {
      dest: dest_path.to_path_buf(),
      size: expected_size,
      sha1: param.sha1.clone(),
      sources: vec![source.clone()],
      resume_offset: offset,
    };
    let engine = SequentialEngine::new()
      .with_limiter(limiter.clone())
      .with_headers_timeout(headers_timeout)
      .with_sha1_retry_on_same_source(retry_same_source_on_sha1);
    let mut sink: Box<dyn ProgressSink> = match task_handle.clone() {
      Some(handle) => Box::new(TaskProgressSink::new(handle)),
      None => Box::new(NullProgressSink::new()),
    };

    match engine.fetch(client, &plan, sink.as_mut()).await {
      Ok(FetchOutcome::Completed) => {
        source_health().record_success(&host, started.elapsed());
        return Ok(());
      }
      Ok(FetchOutcome::Cancelled) => {
        discard_empty_leftover(dest_path).await;
        return Ok(());
      }
      Err(error) => {
        let failures = source_health().record_failure(&host);
        log::warn!(
          "Download source {source} failed ({failures} consecutive): {}",
          error.0
        );
        attempts.push(format!("{source}: {}", error.0));
        last_error = Some(error.0);
        // Partial bytes belong to this source only; the next one may not honour
        // the same range, so drop them instead of risking a spliced file.
        let _ = tokio::fs::remove_file(dest_path).await;
      }
    }
  }

  // Reaching this point means at least one candidate was really contacted: the
  // fail-open branch above guarantees the loop cannot end on skips alone, so
  // this error always counts and names sources that were actually tried.
  Err(SJMCLError(match last_error {
    Some(detail) => format!(
      "Download failed after trying {attempted} source(s): {detail}. Tried: {}",
      attempts.join(" | ")
    ),
    None => format!(
      "Download failed: no candidate source could be attempted for {}",
      param.src
    ),
  }))
}

/// Bridges the engine's [`ProgressSink`] callbacks to a task handle.
///
/// This replaces the `ProgressStream` wrapper that used to sit inside the
/// transfer path: it keeps the same report interval, pause parking and
/// completion bookkeeping, so task events are emitted exactly as before. The
/// engine reports only what happened to the bytes; this adapter is the one place
/// that knows how a launcher task stores and publishes them.
struct TaskProgressSink {
  handle: Arc<RwLock<PTaskHandle>>,
  report_interval: Duration,
  next_report: tokio::time::Instant,
  pending: i64,
}

impl TaskProgressSink {
  fn new(handle: Arc<RwLock<PTaskHandle>>) -> Self {
    let report_interval = handle.read().unwrap().report_interval;
    Self {
      handle,
      report_interval,
      // Report the first chunk immediately, like the previous interval
      // implementation did, so the UI switches to "in progress" right away.
      next_report: tokio::time::Instant::now(),
      pending: 0,
    }
  }
}

impl ProgressSink for TaskProgressSink {
  fn on_bytes(&mut self, delta: u64) {
    self.pending += delta as i64;
    if self.next_report <= tokio::time::Instant::now() {
      let incr = std::mem::take(&mut self.pending);
      let mut handle = self.handle.write().unwrap();
      handle.add_progress(incr);
      handle.flush_progress();
      self.next_report = tokio::time::Instant::now() + self.report_interval;
    }
  }

  fn on_total(&mut self, total: i64) {
    self.handle.write().unwrap().set_total(total);
  }

  fn on_started(&mut self) {
    self.handle.write().unwrap().mark_started();
  }

  fn is_cancelled(&self) -> bool {
    self.handle.read().unwrap().status().is_cancelled()
  }

  fn wait_if_paused(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(std::future::poll_fn(|cx| {
      let mut handle = self.handle.write().unwrap();
      if !handle.status().is_stopped() {
        return std::task::Poll::Ready(());
      }
      // Persist bytes received since the last report before parking, then store
      // the waker so `mark_resumed` can wake the transfer.
      let incr = std::mem::take(&mut self.pending);
      handle.store_unreported_progress(incr);
      handle.store_waker(cx.waker().clone());
      std::task::Poll::Pending
    }))
  }

  fn on_complete(&mut self) {
    let incr = std::mem::take(&mut self.pending);
    let mut handle = self.handle.write().unwrap();
    handle.add_progress(incr);
    handle.mark_completed();
  }
}

impl DownloadTask {
  pub fn new(
    app_handle: AppHandle,
    task_id: u32,
    task_group: Option<String>,
    param: DownloadParam,
    report_interval: Duration,
  ) -> Self {
    let cache_dir = download_cache_dir(&app_handle);
    DownloadTask {
      p_handle: PTaskHandle::new(
        PDesc::<PTaskParam>::new(
          task_id,
          task_group.clone(),
          0,
          PTaskParam::Download(param.clone()),
          PStatus::InProgress,
        ),
        Duration::from_secs(1),
        cache_dir.clone().join(format!("task-{task_id}.json")),
        Reporter::new(
          0,
          Duration::from_secs(1),
          TauriEventSink::new(app_handle.clone()),
        ),
      ),
      alt_srcs: derive_alt_sources(&app_handle, &param.src),
      // A manifest size is only present when the caller could supply one.
      expected_size: param.size,
      param: param.clone(),
      dest_path: cache_dir.clone().join(param.dest.clone()),
      report_interval,
    }
  }

  pub fn from_descriptor(
    app_handle: AppHandle,
    desc: PTaskDesc,
    report_interval: Duration,
    reset: bool,
  ) -> Self {
    let param = match &desc.payload {
      PTaskParam::Download(param) => param.clone(),
    };

    let cache_dir = download_cache_dir(&app_handle);
    let task_id = desc.task_id;
    let path = cache_dir.join(format!("task-{task_id}.json"));
    DownloadTask {
      p_handle: PTaskHandle::new(
        if reset {
          PTaskDesc {
            status: PStatus::Waiting,
            current: 0,
            ..desc
          }
        } else {
          PTaskDesc {
            status: PStatus::Waiting,
            ..desc
          }
        },
        Duration::from_secs(1),
        path,
        Reporter::new(
          desc.total,
          Duration::from_secs(1),
          TauriEventSink::new(app_handle.clone()),
        ),
      ),
      alt_srcs: derive_alt_sources(&app_handle, &param.src),
      // A manifest size is only present when the caller could supply one.
      expected_size: param.size,
      param: param.clone(),
      dest_path: cache_dir.clone().join(param.dest.clone()),
      report_interval,
    }
  }

  async fn future_impl(
    self,
    app_handle: AppHandle,
    limiter: Option<Limiter>,
  ) -> SJMCLResult<(
    impl Future<Output = SJMCLResult<()>> + Send,
    Arc<RwLock<PTaskHandle>>,
  )> {
    let expected_size = self.expected_size;
    let current = safe_resume_offset(self.p_handle.desc.current, expected_size);
    let alt_srcs = self.alt_srcs.clone();
    let handle = Arc::new(RwLock::new(self.p_handle));
    let task_handle = handle.clone();
    let param = self.param.clone();
    Ok((
      async move {
        let client = download_client(&app_handle);
        // Failover stays here; the per-source transfer is delegated by
        // `download_with_policy` to `SequentialEngine`, so no transfer loop
        // lives in the task itself.
        download_with_policy(
          client,
          &param,
          &alt_srcs,
          expected_size,
          &self.dest_path,
          current,
          limiter,
          Some(task_handle),
        )
        .await
      },
      handle,
    ))
  }

  pub async fn future(
    self,
    app_handle: AppHandle,
    limiter: Option<Limiter>,
  ) -> SJMCLResult<(
    impl Future<Output = SJMCLResult<()>> + Send,
    Arc<RwLock<PTaskHandle>>,
  )> {
    Self::future_impl(self, app_handle, limiter).await
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use sha1::{Digest, Sha1};
  use std::pin::Pin;
  use std::task::{Context, Poll};
  use tokio::io::AsyncWrite;

  struct CountingWriter {
    writes: Vec<usize>,
    total: usize,
  }

  impl AsyncWrite for CountingWriter {
    fn poll_write(
      mut self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
      self.writes.push(buf.len());
      self.total += buf.len();
      Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
      Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
      Poll::Ready(Ok(()))
    }
  }

  pub(crate) fn unique_temp_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    std::env::temp_dir().join(format!(
      "sjmcl-download-{}-{}-{}",
      tag,
      std::process::id(),
      nanos
    ))
  }

  #[tokio::test]
  async fn copy_to_file_batches_writes_into_download_buffer() {
    const TOTAL: usize = 300_000;
    let data = vec![0x5a_u8; TOTAL];
    let mut reader = &data[..];
    let mut writer = CountingWriter {
      writes: Vec::new(),
      total: 0,
    };

    let copied = copy_to_file(&mut reader, &mut writer).await.unwrap();

    assert_eq!(copied as usize, TOTAL);
    assert_eq!(writer.total, TOTAL);
    // Four full 64 KiB writes plus the tail, instead of ~37 writes of 8 KiB.
    assert_eq!(
      writer.writes,
      vec![
        DOWNLOAD_BUFFER_SIZE,
        DOWNLOAD_BUFFER_SIZE,
        DOWNLOAD_BUFFER_SIZE,
        DOWNLOAD_BUFFER_SIZE,
        TOTAL - 4 * DOWNLOAD_BUFFER_SIZE
      ]
    );
  }

  #[test]
  fn download_buffer_size_is_64_kib() {
    assert_eq!(DOWNLOAD_BUFFER_SIZE, 64 * 1024);
  }

  #[tokio::test]
  async fn prepare_dest_file_resumes_at_persisted_offset() {
    let path = unique_temp_path("resume");
    std::fs::write(&path, b"0123456789").unwrap();

    let mut file = prepare_dest_file(&path, 4).await.unwrap();
    file.write_all(b"XY").await.unwrap();
    file.flush().await.unwrap();
    drop(file);
    // Bytes before the resumed offset are kept byte-for-byte.
    assert_eq!(std::fs::read(&path).unwrap(), b"0123XY6789");

    // A fresh download (current == 0) still truncates a leftover file.
    let file = prepare_dest_file(&path, 0).await.unwrap();
    assert_eq!(file.metadata().await.unwrap().len(), 0);
    drop(file);

    let _ = std::fs::remove_file(&path);
  }

  /// Reads an HTTP/1.1 request head byte by byte. Returns false when the peer
  /// closed the connection before sending a complete head.
  async fn read_request_head(socket: &mut tokio::net::TcpStream) -> bool {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
      match socket.read(&mut byte).await {
        Ok(0) => return false,
        Ok(_) => {
          head.push(byte[0]);
          if head.ends_with(b"\r\n\r\n") {
            return true;
          }
        }
        Err(_) => return false,
      }
    }
  }

  /// Serves `total` bytes over two sequential connections, `chunk` bytes every
  /// `delay`, so a test can compare timeout policies without touching the
  /// public network. Returns the URL of the first connection.
  async fn spawn_slow_server(total: usize, chunk: usize, delay: Duration) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
      for _ in 0..2 {
        let Ok((mut socket, _)) = listener.accept().await else {
          return;
        };
        tokio::spawn(async move {
          if !read_request_head(&mut socket).await {
            return;
          }
          let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
          );
          if socket.write_all(header.as_bytes()).await.is_err() {
            return;
          }
          let mut remaining = total;
          while remaining > 0 {
            let len = chunk.min(remaining);
            if socket.write_all(&vec![b'x'; len]).await.is_err() {
              return;
            }
            remaining -= len;
            if remaining > 0 {
              tokio::time::sleep(delay).await;
            }
          }
          let _ = socket.shutdown().await;
        });
      }
    });
    format!("http://{address}/file.bin")
  }

  #[tokio::test]
  async fn total_deadline_truncates_slow_transfers_but_download_client_finishes_them() {
    const TOTAL: usize = 16 * 1024;
    let url = spawn_slow_server(TOTAL, 1024, Duration::from_millis(25)).await;

    // Reproduce the shared client's behaviour with a scaled-down 10 s total
    // deadline: reqwest applies it to the response body too, so the stream is
    // aborted before the advertised Content-Length is reached.
    let legacy = Client::builder()
      .timeout(Duration::from_millis(200))
      .no_proxy()
      .build()
      .unwrap();
    let response = legacy.get(&url).send().await.unwrap();
    let mut received = 0usize;
    let mut timed_out = false;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
      match chunk {
        Ok(bytes) => received += bytes.len(),
        Err(_) => {
          timed_out = true;
          break;
        }
      }
    }
    println!(
      "shared client (200 ms simulated total deadline): received {received}/{TOTAL} bytes, timed_out={timed_out}"
    );
    assert!(timed_out, "the total deadline must abort the slow body");
    assert!(
      received < TOTAL,
      "the shared client must not receive the whole file"
    );

    // The dedicated download client has no total deadline and completes the
    // same slow transfer that the shared client truncated.
    let client = download_client_builder().no_proxy().build().unwrap();
    let response = client.get(&url).send().await.unwrap();
    let body = response.bytes().await.unwrap();
    println!(
      "download client (read timeout, no total deadline): received {}/{TOTAL} bytes",
      body.len()
    );
    assert_eq!(body.len(), TOTAL);
  }

  #[tokio::test]
  async fn download_client_reuses_keep_alive_connections() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let accepted_by_server = accepted.clone();
    tokio::spawn(async move {
      loop {
        let Ok((mut socket, _)) = listener.accept().await else {
          return;
        };
        accepted_by_server.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(async move {
          loop {
            if !read_request_head(&mut socket).await {
              return;
            }
            if socket
              .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok",
              )
              .await
              .is_err()
            {
              return;
            }
          }
        });
      }
    });
    let url = format!("http://{address}/file.bin");

    let client = download_client_builder().no_proxy().build().unwrap();
    for _ in 0..3 {
      let body = client.get(&url).send().await.unwrap().text().await.unwrap();
      assert_eq!(body, "ok");
    }

    let connections = accepted.load(std::sync::atomic::Ordering::SeqCst);
    println!("3 sequential downloads used {connections} TCP connection(s)");
    assert_eq!(
      connections, 1,
      "three consecutive requests must reuse one pooled HTTP/1.1 connection"
    );
  }

  #[tokio::test]
  async fn truncated_body_surfaces_an_error_instead_of_a_silent_eof() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
      let Ok((mut socket, _)) = listener.accept().await else {
        return;
      };
      if !read_request_head(&mut socket).await {
        return;
      }
      let _ = socket
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n")
        .await;
      let _ = socket.write_all(&[b'x'; 128]).await;
      let _ = socket.shutdown().await;
    });
    let url = format!("http://{address}/file.bin");

    let client = download_client_builder().no_proxy().build().unwrap();
    let response = client.get(&url).send().await.unwrap();
    let mut stream = Box::pin(into_byte_stream(response));
    let mut received = 0usize;
    let mut error = None;
    while let Some(chunk) = stream.next().await {
      match chunk {
        Ok(bytes) => received += bytes.len(),
        Err(e) => {
          error = Some(e);
          break;
        }
      }
    }

    println!("truncated body: received {received}/1024 bytes before the stream error");
    assert!(received < 1024);
    let error = error.expect("a body shorter than Content-Length must not look like EOF");
    assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
  }

  #[test]
  fn resumed_downloads_require_partial_content() {
    assert!(ensure_resumed_response(0, StatusCode::OK).is_ok());
    assert!(ensure_resumed_response(0, StatusCode::PARTIAL_CONTENT).is_ok());
    assert!(ensure_resumed_response(1024, StatusCode::PARTIAL_CONTENT).is_ok());
    // A server that ignores the Range header would otherwise append the whole
    // file after the persisted prefix.
    assert!(ensure_resumed_response(1024, StatusCode::OK).is_err());
    // Negative offsets are clamped before this check, so they are fresh files.
    assert!(ensure_resumed_response(0, StatusCode::OK).is_ok());
  }

  #[tokio::test]
  async fn header_deadline_fails_when_server_never_responds() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
      let Ok((mut socket, _)) = listener.accept().await else {
        return;
      };
      if !read_request_head(&mut socket).await {
        return;
      }
      // Accept the request, then stay silent without sending any header.
      tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let url = format!("http://{address}/file.bin");

    let client = download_client_builder().no_proxy().build().unwrap();
    let started = std::time::Instant::now();
    let result = send_with_retry_budget(
      Duration::from_millis(150),
      3,
      Duration::from_secs(30),
      || {
        let client = client.clone();
        let url = url.clone();
        async move {
          client
            .get(url)
            .send()
            .await
            .map_err(DownloadRequestError::Request)
        }
      },
    )
    .await;

    println!(
      "silent server: failed={} after {:?}",
      result.is_err(),
      started.elapsed()
    );
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
  }

  #[test]
  fn resume_offset_clamps_corrupt_descriptor_values() {
    assert_eq!(resume_offset(0), 0);
    assert_eq!(resume_offset(-1), 0);
    assert_eq!(resume_offset(i64::MIN), 0);
    assert_eq!(resume_offset(42), 42);
  }

  #[test]
  fn safe_resume_offset_restarts_when_the_leftover_is_not_shorter_than_the_target() {
    assert_eq!(safe_resume_offset(0, None), 0);
    assert_eq!(safe_resume_offset(-7, Some(2048)), 0);
    assert_eq!(safe_resume_offset(1234, Some(4096)), 1234);
    // A complete or oversized leftover cannot be a prefix of the target file.
    assert_eq!(safe_resume_offset(4096, Some(4096)), 0);
    assert_eq!(safe_resume_offset(9000, Some(4096)), 0);
    // Without an announced size the persisted offset is kept as before.
    assert_eq!(safe_resume_offset(9000, None), 9000);
  }

  #[tokio::test]
  async fn cancellation_discards_only_an_empty_leftover() {
    let empty = unique_temp_path("cancel-empty");
    std::fs::write(&empty, b"").unwrap();
    discard_empty_leftover(&empty).await;
    assert!(!empty.exists());

    // A missing file stays missing: cleanup is idempotent.
    discard_empty_leftover(&empty).await;

    let partial = unique_temp_path("cancel-partial");
    std::fs::write(&partial, b"partial").unwrap();
    discard_empty_leftover(&partial).await;
    assert_eq!(std::fs::read(&partial).unwrap(), b"partial");
    let _ = std::fs::remove_file(&partial);
  }

  #[tokio::test]
  async fn validate_dest_sha1_checks_written_file() {
    let path = unique_temp_path("sha1");
    std::fs::write(&path, b"hello").unwrap();
    let mut hasher = Sha1::new();
    hasher.update(b"hello");
    let digest = hex::encode(hasher.finalize());

    assert!(
      validate_dest_sha1(path.clone(), digest.clone())
        .await
        .is_ok()
    );
    assert!(
      validate_dest_sha1(path.clone(), "0".repeat(40))
        .await
        .is_err()
    );

    let missing = unique_temp_path("sha1-missing");
    assert!(validate_dest_sha1(missing, digest).await.is_err());

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn download_failures_are_classified_for_the_retry_policy() {
    let client = Client::builder().no_proxy().build().unwrap();

    // Connection refused: the mirror is unreachable, retrying it is pointless.
    let connect_error = client
      .get("http://127.0.0.1:1/file.bin")
      .send()
      .await
      .expect_err("connecting to port 1 must fail");
    assert!(connect_error.is_connect(), "{connect_error}");
    assert_eq!(
      classify_download_failure(DownloadFailure::Request(&DownloadRequestError::Request(
        connect_error
      ))),
      DownloadFailureClass::FastFail
    );

    // The per-attempt header deadline is fast-fail as well.
    assert_eq!(
      classify_download_failure(DownloadFailure::Request(&DownloadRequestError::Timeout)),
      DownloadFailureClass::FastFail
    );

    // A real reqwest timeout (server accepts the request but never answers).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
      let Ok((mut socket, _)) = listener.accept().await else {
        return;
      };
      let _ = read_request_head(&mut socket).await;
      tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let timeout_client = Client::builder()
      .no_proxy()
      .timeout(Duration::from_millis(100))
      .build()
      .unwrap();
    let timeout_error = timeout_client
      .get(format!("http://{address}/file.bin"))
      .send()
      .await
      .expect_err("the silent server must time out");
    assert!(timeout_error.is_timeout(), "{timeout_error}");
    assert_eq!(
      classify_download_failure(DownloadFailure::Request(&DownloadRequestError::Request(
        timeout_error
      ))),
      DownloadFailureClass::FastFail
    );

    // Body interruptions and short reads are the failures a resume can fix.
    for kind in [
      std::io::ErrorKind::ConnectionReset,
      std::io::ErrorKind::ConnectionAborted,
      std::io::ErrorKind::BrokenPipe,
      std::io::ErrorKind::UnexpectedEof,
    ] {
      let error = std::io::Error::new(kind, "body interrupted");
      assert_eq!(
        classify_download_failure(DownloadFailure::Body(&error)),
        DownloadFailureClass::Retryable,
        "{kind:?} must be retryable"
      );
    }

    // A local filesystem error is not a network failure; do not loop on it.
    let local = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    assert_eq!(
      classify_download_failure(DownloadFailure::Body(&local)),
      DownloadFailureClass::FastFail
    );
  }

  #[tokio::test]
  async fn refused_connection_fails_fast_within_five_seconds() {
    let client = download_client_builder().no_proxy().build().unwrap();
    let param = DownloadParam {
      src: Url::parse("http://127.0.0.1:1/file.bin").unwrap(),
      dest: PathBuf::from("unreachable.bin"),
      filename: None,
      sha1: None,
      size: None,
    };

    let started = std::time::Instant::now();
    let result =
      SequentialEngine::send_request(&client, 0, &param.src, Duration::from_secs(30)).await;
    let elapsed = started.elapsed();

    // #1942 evidence: a refused mirror must not consume the one hour global
    // retry budget; the download path gives up within the five second budget.
    println!("refused connection failed in {elapsed:?}");
    assert!(result.is_err());
    assert!(
      elapsed < Duration::from_secs(5),
      "refused connection took {elapsed:?}, expected the download path to give up in under 5 s"
    );
  }

  pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
  }

  /// One canned HTTP/1.1 reply, served on its own TCP connection.
  pub(crate) enum FakeReply {
    /// Advertise the full body length but write only `sent` bytes before
    /// closing: the client sees a short read / connection reset.
    TruncatedBody { body: Vec<u8>, sent: usize },
    /// No Content-Length: the connection close delimits the body, so a short
    /// write is a clean EOF (the silent truncation observed on the mirror).
    BodyEndedByClose { body: Vec<u8> },
    /// `206 Partial Content` with the tail starting at `offset`.
    RangeTail { body: Vec<u8>, offset: usize },
    /// Accepts the request but stays silent for `delay` before sending any
    /// response header, then serves `body`. Reproduces a mirror that stalls
    /// instead of failing immediately (BMCLAPI's "timed out waiting for
    /// response headers" case).
    Delayed { delay: Duration, body: Vec<u8> },
    /// An error status with an empty body, e.g. a dead mirror path.
    Status { code: u16, reason: &'static str },
  }

  pub(crate) struct FakeHttpServer {
    pub(crate) url: String,
    pub(crate) request_heads: Arc<Mutex<Vec<String>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
  }

  impl FakeHttpServer {
    pub(crate) fn spawn(replies: Vec<FakeReply>) -> Self {
      let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
      let address = listener.local_addr().unwrap();
      listener.set_nonblocking(true).unwrap();
      let request_heads = Arc::new(Mutex::new(Vec::new()));
      let heads = request_heads.clone();
      let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
      let stop_by_server = stop.clone();
      let handle = std::thread::spawn(move || {
        let mut index = 0;
        while index < replies.len() && !stop_by_server.load(std::sync::atomic::Ordering::SeqCst) {
          match listener.accept() {
            Ok((mut socket, _)) => {
              let _ = socket.set_nonblocking(false);
              let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
              let head = read_std_request_head(&mut socket);
              heads.lock().unwrap().push(head);
              if let FakeReply::Delayed { delay, .. } = &replies[index] {
                wait_unless_stopped(*delay, &stop_by_server);
              }
              serve_std_reply(&mut socket, &replies[index]);
              index += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
              std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return,
          }
        }
      });

      Self {
        url: format!("http://{address}/file.bin"),
        request_heads,
        stop,
        handle: Some(handle),
      }
    }
  }

  impl Drop for FakeHttpServer {
    fn drop(&mut self) {
      self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
      if let Some(handle) = self.handle.take() {
        let _ = handle.join();
      }
    }
  }

  fn read_std_request_head(socket: &mut std::net::TcpStream) -> String {
    use std::io::Read;

    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
      match socket.read(&mut byte) {
        Ok(0) | Err(_) => return String::from_utf8_lossy(&head).into_owned(),
        Ok(_) => {
          head.push(byte[0]);
          if head.ends_with(b"\r\n\r\n") {
            return String::from_utf8_lossy(&head).into_owned();
          }
        }
      }
    }
  }

  /// Sleeps for `delay` before a reply, but wakes up as soon as the server is
  /// dropped so a stalled reply never holds up test teardown for its whole
  /// delay.
  fn wait_unless_stopped(delay: Duration, stop: &std::sync::atomic::AtomicBool) {
    let started = std::time::Instant::now();
    while started.elapsed() < delay {
      if stop.load(std::sync::atomic::Ordering::SeqCst) {
        return;
      }
      std::thread::sleep((delay - started.elapsed()).min(Duration::from_millis(10)));
    }
  }

  fn serve_std_reply(socket: &mut std::net::TcpStream, reply: &FakeReply) {
    use std::io::Write;

    match reply {
      FakeReply::TruncatedBody { body, sent } => {
        let head = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          body.len()
        );
        let _ = socket.write_all(head.as_bytes());
        let _ = socket.write_all(&body[..(*sent).min(body.len())]);
      }
      FakeReply::BodyEndedByClose { body } => {
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
        let _ = socket.write_all(body);
      }
      FakeReply::Delayed { body, .. } => {
        let head = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          body.len()
        );
        let _ = socket.write_all(head.as_bytes());
        let _ = socket.write_all(body);
      }
      FakeReply::Status { code, reason } => {
        let response =
          format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let _ = socket.write_all(response.as_bytes());
      }
      FakeReply::RangeTail { body, offset } => {
        let tail = &body[*offset..];
        let head = format!(
          "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
          tail.len(),
          offset,
          body.len() - 1,
          body.len()
        );
        let _ = socket.write_all(head.as_bytes());
        let _ = socket.write_all(tail);
      }
    }
    let _ = socket.shutdown(std::net::Shutdown::Write);
  }

  #[tokio::test]
  async fn sha1_retry_recovers_from_a_truncated_first_body() {
    let payload: Vec<u8> = (0..8192).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    let server = FakeHttpServer::spawn(vec![
      FakeReply::BodyEndedByClose {
        body: payload[..1024].to_vec(),
      },
      FakeReply::BodyEndedByClose {
        body: payload.clone(),
      },
    ]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("sha1-truncated");
    let param = DownloadParam {
      src: Url::parse(&server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: None,
    };

    download_with_policy(&client, &param, &[], None, &dest, 0, None, None)
      .await
      .expect("the truncated first body must be retried and succeed");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(validate_dest_sha1(dest.clone(), truth).await, Ok(()));
    let heads = server.request_heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 2);
    assert!(
      !heads[1].to_ascii_lowercase().contains("range:"),
      "the retry must restart from scratch, not resume the truncated file"
    );
    println!("sha1 retry: succeeded after 2 attempts; retry had no Range header");
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn a_dead_first_source_falls_over_to_the_next_one() {
    let payload: Vec<u8> = (0..4096).map(|index| (index % 241) as u8).collect();
    let truth = sha1_hex(&payload);
    let dead = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 404,
      reason: "Not Found",
    }]);
    let alive = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("failover");
    let param = DownloadParam {
      src: Url::parse(&dead.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: None,
    };
    let alt = [Url::parse(&alive.url).unwrap()];

    download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect("the healthy second source must be used");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(validate_dest_sha1(dest.clone(), truth).await, Ok(()));
    assert_eq!(dead.request_heads.lock().unwrap().len(), 1);
    assert_eq!(alive.request_heads.lock().unwrap().len(), 1);
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn stalled_secondary_is_fast_failed_then_the_next_source_downloads() {
    // Real-world shape (BMCLAPI fabric-maven): the primary answers 404 at once,
    // the secondary accepts the request but never sends response headers, and
    // the tertiary mirror serves the file. The stalled source must be abandoned
    // by the fast-fail budget, not waited on until its own header deadline.
    const STALL: Duration = Duration::from_secs(6);
    let payload: Vec<u8> = (0..4096).map(|index| (index % 241) as u8).collect();
    let truth = sha1_hex(&payload);
    let primary = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 404,
      reason: "Not Found",
    }]);
    let stalled = FakeHttpServer::spawn(vec![FakeReply::Delayed {
      delay: STALL,
      // If the downloader waited the stall out instead of failing over, this
      // body would satisfy the SHA-1 and hide the regression, so the
      // request-count assertions below also prove the tertiary was used.
      body: payload.clone(),
    }]);
    let tertiary = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    // Ephemeral ports can be recycled between tests and the health map is
    // process-global; re-arm all three hosts so a stale disabled entry cannot
    // turn the stall into a skip.
    for server in [&primary, &stalled, &tertiary] {
      let host = SourceHealth::host_key(&Url::parse(&server.url).unwrap());
      source_health().record_success(&host, Duration::from_millis(5));
    }

    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("stalled-failover");
    let param = DownloadParam {
      src: Url::parse(&primary.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: None,
    };
    let alt = [
      Url::parse(&stalled.url).unwrap(),
      Url::parse(&tertiary.url).unwrap(),
    ];

    let started = std::time::Instant::now();
    download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect("the third source must serve the file after the stalled second one");
    let elapsed = started.elapsed();

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(validate_dest_sha1(dest.clone(), truth).await, Ok(()));
    assert_eq!(primary.request_heads.lock().unwrap().len(), 1);
    assert_eq!(stalled.request_heads.lock().unwrap().len(), 1);
    assert_eq!(tertiary.request_heads.lock().unwrap().len(), 1);
    println!("failover done in {elapsed:?}; stall {STALL:?}; requests 1/1/1; tertiary bytes 4096");
    assert!(
      elapsed < Duration::from_secs(20),
      "failover from the stalled source took {elapsed:?}, expected under 20 s"
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn every_source_failing_reports_what_was_tried() {
    let first = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 500,
      reason: "Internal Server Error",
    }]);
    let second = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 404,
      reason: "Not Found",
    }]);
    let third = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 403,
      reason: "Forbidden",
    }]);
    let first_url = Url::parse(&first.url).unwrap();
    let second_url = Url::parse(&second.url).unwrap();
    let third_url = Url::parse(&third.url).unwrap();
    for url in [&first_url, &second_url, &third_url] {
      source_health().record_success(&SourceHealth::host_key(url), Duration::from_millis(5));
    }
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("all-sources-fail");
    let param = DownloadParam {
      src: first_url,
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: None,
      size: None,
    };
    let alt = [second_url, third_url];

    let error = download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect_err("all three sources fail, so the task must fail");

    assert!(error.0.contains("after trying 3 source(s)"), "{}", error.0);
    assert!(error.0.contains(&first.url), "{}", error.0);
    assert!(error.0.contains(&second.url), "{}", error.0);
    assert!(error.0.contains(&third.url), "{}", error.0);
    assert_eq!(first.request_heads.lock().unwrap().len(), 1);
    assert_eq!(second.request_heads.lock().unwrap().len(), 1);
    assert_eq!(third.request_heads.lock().unwrap().len(), 1);
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn all_sources_unhealthy_fails_open_and_downloads() {
    // Regression: when a past outage disabled every candidate, the old code
    // returned an error without sending a single request, so the task kept
    // failing for the whole TTL even after the mirror had recovered.
    let payload: Vec<u8> = (0..4096).map(|index| (index % 233) as u8).collect();
    let truth = sha1_hex(&payload);
    let first = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let second = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let first_url = Url::parse(&first.url).unwrap();
    let second_url = Url::parse(&second.url).unwrap();
    let first_host = SourceHealth::host_key(&first_url);
    let second_host = SourceHealth::host_key(&second_url);
    for _ in 0..3 {
      source_health().record_failure(&first_host);
      source_health().record_failure(&second_host);
    }
    assert!(source_health().is_disabled(&first_host));
    assert!(source_health().is_disabled(&second_host));

    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("fail-open");
    let param = DownloadParam {
      src: first_url,
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: None,
    };
    let alt = [second_url];

    download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect("every source being unhealthy must fail open, not fail the task");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(validate_dest_sha1(dest.clone(), truth).await, Ok(()));
    let requests =
      first.request_heads.lock().unwrap().len() + second.request_heads.lock().unwrap().len();
    assert!(
      requests >= 1,
      "fail-open must actually send a request, got {requests}"
    );
    println!("fail-open with all sources unhealthy: {requests} request(s) sent");
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn unhealthy_primary_is_skipped_in_favour_of_healthy_alt() {
    let payload: Vec<u8> = (0..2048).map(|index| (index % 197) as u8).collect();
    let truth = sha1_hex(&payload);
    let primary = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let alt = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let primary_url = Url::parse(&primary.url).unwrap();
    let alt_url = Url::parse(&alt.url).unwrap();
    let primary_host = SourceHealth::host_key(&primary_url);
    let alt_host = SourceHealth::host_key(&alt_url);
    // Re-enable the alternative first so a recycled port from another test can
    // never leak state into this assertion; then disable only the primary.
    source_health().record_success(&alt_host, std::time::Duration::from_millis(5));
    for _ in 0..3 {
      source_health().record_failure(&primary_host);
    }
    assert!(source_health().is_disabled(&primary_host));
    assert!(!source_health().is_disabled(&alt_host));

    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("skip-unhealthy-primary");
    let param = DownloadParam {
      src: primary_url,
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: None,
    };
    let alt_srcs = [alt_url];

    download_with_policy(&client, &param, &alt_srcs[..], None, &dest, 0, None, None)
      .await
      .expect("the healthy alternative must serve the download");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(primary.request_heads.lock().unwrap().len(), 0);
    assert_eq!(alt.request_heads.lock().unwrap().len(), 1);
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn single_unhealthy_source_is_still_tried() {
    // Boundary: one candidate, and that candidate is disabled. Fail-open must
    // still send the request instead of reporting "unhealthy" without trying.
    let payload: Vec<u8> = (0..512).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    let server = FakeHttpServer::spawn(vec![FakeReply::BodyEndedByClose {
      body: payload.clone(),
    }]);
    let url = Url::parse(&server.url).unwrap();
    let host = SourceHealth::host_key(&url);
    for _ in 0..3 {
      source_health().record_failure(&host);
    }
    assert!(source_health().is_disabled(&host));

    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("fail-open-single");
    let param = DownloadParam {
      src: url,
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth),
      size: None,
    };

    download_with_policy(&client, &param, &[], None, &dest, 0, None, None)
      .await
      .expect("a single disabled source must still be tried");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(server.request_heads.lock().unwrap().len(), 1);
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn all_sources_unhealthy_and_failing_still_reports_attempts() {
    let first = FakeHttpServer::spawn(vec![
      FakeReply::Status {
        code: 500,
        reason: "Internal Server Error",
      },
      FakeReply::Status {
        code: 500,
        reason: "Internal Server Error",
      },
      FakeReply::Status {
        code: 500,
        reason: "Internal Server Error",
      },
    ]);
    let second = FakeHttpServer::spawn(vec![
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
    let first_url = Url::parse(&first.url).unwrap();
    let second_url = Url::parse(&second.url).unwrap();
    for _ in 0..3 {
      source_health().record_failure(&SourceHealth::host_key(&first_url));
      source_health().record_failure(&SourceHealth::host_key(&second_url));
    }
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("fail-open-all-fail");
    let param = DownloadParam {
      src: first_url,
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: None,
      size: None,
    };
    let alt = [second_url];

    let error = download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect_err("fail-open still reports an error when the retried sources fail");

    assert!(error.0.contains("after trying 2 source(s)"), "{}", error.0);
    assert!(error.0.contains(&first.url), "{}", error.0);
    assert!(error.0.contains(&second.url), "{}", error.0);
    assert!(
      first.request_heads.lock().unwrap().len() >= 1,
      "the primary must have been contacted under fail-open"
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn sha1_retry_exhaustion_fails_and_removes_the_partial_file() {
    let payload: Vec<u8> = (0..4096).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    let truncated = payload[..512].to_vec();
    let attempts = 1 + DOWNLOAD_SHA1_RETRIES;
    let replies = (0..attempts)
      .map(|_| FakeReply::BodyEndedByClose {
        body: truncated.clone(),
      })
      .collect();
    let server = FakeHttpServer::spawn(replies);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("sha1-exhausted");
    let param = DownloadParam {
      src: Url::parse(&server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth),
      size: None,
    };

    let result = download_with_policy(&client, &param, &[], None, &dest, 0, None, None).await;

    assert!(result.is_err(), "exhausted SHA1 retries must fail the task");
    assert_eq!(server.request_heads.lock().unwrap().len(), attempts);
    assert!(
      !dest.exists(),
      "an unverified partial file must not survive exhausted retries"
    );
    println!("sha1 retry exhaustion: {attempts} attempts, no file left behind");
  }

  #[tokio::test]
  async fn interrupted_body_resumes_from_the_bytes_already_on_disk() {
    let payload: Vec<u8> = (0..8192).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    let server = FakeHttpServer::spawn(vec![
      FakeReply::TruncatedBody {
        body: payload.clone(),
        sent: 2048,
      },
      FakeReply::RangeTail {
        body: payload.clone(),
        offset: 2048,
      },
    ]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("resume-body");
    let param = DownloadParam {
      src: Url::parse(&server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth),
      size: None,
    };

    download_with_policy(&client, &param, &[], None, &dest, 0, None, None)
      .await
      .expect("an interrupted body must be resumed with a Range request");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    let heads = server.request_heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 2);
    assert!(
      heads[1].to_ascii_lowercase().contains("range: bytes=2048-"),
      "the resume request must start at the byte offset already on disk, got: {}",
      heads[1]
    );
    println!("body resume: second request used Range: bytes=2048-");
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn wrong_content_length_is_rejected_and_the_next_source_is_used() {
    let payload: Vec<u8> = (0..4096).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    // The first source announces 8192 bytes for a 4096-byte artifact and does
    // send a body: the precheck must reject it before any byte is written.
    let wrong_size = FakeHttpServer::spawn(vec![FakeReply::TruncatedBody {
      body: vec![0xee; 8192],
      sent: 8192,
    }]);
    let healthy = FakeHttpServer::spawn(vec![FakeReply::TruncatedBody {
      body: payload.clone(),
      sent: payload.len(),
    }]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("size-mismatch");
    let param = DownloadParam {
      src: Url::parse(&wrong_size.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth.clone()),
      size: Some(payload.len() as u64),
    };
    let alt = [Url::parse(&healthy.url).unwrap()];

    download_with_policy(&client, &param, &alt[..], param.size, &dest, 0, None, None)
      .await
      .expect("a source with a wrong announced size must fall over to the next one");

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(validate_dest_sha1(dest.clone(), truth).await, Ok(()));
    // The rejected source was contacted exactly once and none of its bytes
    // reached the file: failover happened at the header stage.
    assert_eq!(wrong_size.request_heads.lock().unwrap().len(), 1);
    assert_eq!(healthy.request_heads.lock().unwrap().len(), 1);
    let _ = std::fs::remove_file(&dest);
  }

  #[tokio::test]
  async fn size_none_disables_the_content_length_check() {
    let payload: Vec<u8> = (0..1024).map(|index| (index % 241) as u8).collect();
    let server = FakeHttpServer::spawn(vec![
      FakeReply::TruncatedBody {
        body: payload.clone(),
        sent: payload.len(),
      },
      FakeReply::TruncatedBody {
        body: payload.clone(),
        sent: payload.len(),
      },
    ]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("size-unknown");
    let param = DownloadParam {
      src: Url::parse(&server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: None,
      size: None,
    };

    // With no declared size there is nothing to compare against, so the
    // announced 1024 bytes are accepted and the transfer proceeds.
    download_with_policy(&client, &param, &[], param.size, &dest, 0, None, None)
      .await
      .expect("size: None must disable the Content-Length precheck");
    assert_eq!(std::fs::read(&dest).unwrap(), payload);

    // The very same response is rejected once a size is declared, proving the
    // previous success came from the disabled precheck rather than a missed
    // comparison.
    let mut declared = param.clone();
    declared.size = Some(4096);
    let error = download_with_policy(&client, &declared, &[], declared.size, &dest, 0, None, None)
      .await
      .expect_err("a declared size must reject the mismatching announcement");
    assert!(
      error
        .0
        .contains("Size mismatch: expected 4096 bytes, the source announced 1024"),
      "{}",
      error.0
    );
    let _ = std::fs::remove_file(&dest);
  }

  #[test]
  fn legacy_download_param_without_size_deserializes() {
    // Persisted task descriptors created before the size field existed must
    // still load: serde(default) turns the missing field into None.
    let json =
      r#"{"src":"http://127.0.0.1:1/file.bin","dest":"file.bin","filename":null,"sha1":null}"#;
    let param: DownloadParam = serde_json::from_str(json).unwrap();
    assert_eq!(param.size, None);
  }

  #[tokio::test]
  async fn resumed_requests_are_not_size_checked() {
    let payload: Vec<u8> = (0..8192).map(|index| (index % 251) as u8).collect();
    let truth = sha1_hex(&payload);
    let server = FakeHttpServer::spawn(vec![FakeReply::RangeTail {
      body: payload.clone(),
      offset: 2048,
    }]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("size-resume");
    std::fs::write(&dest, &payload[..2048]).unwrap();
    let param = DownloadParam {
      src: Url::parse(&server.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: Some(truth),
      size: Some(payload.len() as u64),
    };

    // A 206 response announces only the tail (6144 bytes), not the whole
    // artifact (8192), so the declared size must not be compared here.
    download_with_policy(&client, &param, &[], param.size, &dest, 2048, None, None)
      .await
      .expect("a resumed request must not be rejected by the size precheck");
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    let heads = server.request_heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 1);
    assert!(
      heads[0].to_ascii_lowercase().contains("range: bytes=2048-"),
      "the test must exercise the resumed path, got: {}",
      heads[0]
    );
    let _ = std::fs::remove_file(&dest);
  }
}
