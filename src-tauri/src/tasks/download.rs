use async_speed_limit::Limiter;
use futures::StreamExt;
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_http::reqwest;
use tauri_plugin_http::reqwest::header::{ACCEPT_ENCODING, HeaderMap, HeaderValue, RANGE};
use tauri_plugin_http::reqwest::{Client, ClientBuilder, Proxy, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tokio_util::bytes;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::launcher_config::models::{LauncherConfig, ProxyType};
use crate::resource::helpers::curseforge::misc::{
  CURSEFORGE_API_KEY, is_curseforge_authenticated_url,
};
use crate::resource::helpers::source::candidates;
use crate::tasks::source_health::{SourceHealth, source_health};
use crate::tasks::streams::ProgressStream;
use crate::tasks::streams::desc::{PDesc, PStatus};
use crate::tasks::streams::reporter::Reporter;
use crate::tasks::*;
use crate::utils::fs::validate_sha1;

/// Buffer size used when copying the response stream into the destination file.
///
/// The previous implementation used tokio::io::copy, whose internal buffer is
/// only 8 KiB: every 8 KiB of payload caused one socket read, one file write
/// and one progress-stream poll. 64 KiB matches the sequential download buffer
/// used by PCL2 (80 KiB) and PCL2-CE (64 KiB) and divides the number of
/// syscalls / stream polls per megabyte by 8.
const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;

/// Timeout for establishing the TCP/TLS connection of a download.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum time to wait for the response headers (connect + TLS + redirects +
/// time-to-first-byte). `send()` resolves as soon as the headers arrive, so this
/// deadline never covers the body itself.
const DOWNLOAD_HEADERS_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum time to wait for the next body chunk before treating the transfer as
/// stalled. Unlike a total deadline this resets on every successful read, so a
/// huge file may take as long as it needs while a dead socket fails fast
/// (PCL2-CE uses 15 s for the same purpose).
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Total time the download path spends obtaining response headers before it
/// gives up on a source, so the caller can switch to the next mirror.
///
/// This lives in the download path on purpose: `utils::web::with_retry` keeps
/// its global budget for the other callers, but a mirror that cannot even
/// connect or answer is better abandoned than retried for the one hour global
/// budget (#1942). The value stays below the five second bound promised to the
/// scheduler so the timeout/backoff overhead cannot push the attempt past it.
const DOWNLOAD_FAST_FAIL_BUDGET: Duration = Duration::from_secs(4);

/// Attempts allowed inside `DOWNLOAD_FAST_FAIL_BUDGET`, initial try included.
const DOWNLOAD_FAST_FAIL_ATTEMPTS: usize = 3;

/// Pause between two fast-fail attempts, itself bounded by the remaining budget.
const DOWNLOAD_FAST_FAIL_BACKOFF: Duration = Duration::from_millis(200);

/// How many times an interrupted body is resumed from the bytes already on disk.
const DOWNLOAD_RESUME_RETRIES: usize = 2;

/// How many times a file that fails SHA-1 validation (short body or corrupted
/// transfer) is downloaded again. The partial artifact is deleted before every
/// retry so a truncated file can never be resumed as if it were complete.
const DOWNLOAD_SHA1_RETRIES: usize = 2;

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

/// A resumed download (`Range: bytes=N-`) must be answered with `206 Partial
/// Content`. If a server or proxy ignores the header and returns `200 OK` with
/// the whole file, appending it after the persisted prefix would corrupt the
/// artifact, so fail explicitly instead of silently writing bad bytes.
fn ensure_resumed_response(current: u64, status: StatusCode) -> SJMCLResult<()> {
  if current > 0 && status != StatusCode::PARTIAL_CONTENT {
    return Err(SJMCLError(format!(
      "Server ignored the Range request while resuming (HTTP {}, expected 206)",
      status.as_u16()
    )));
  }
  Ok(())
}

/// Converts the response body into a byte stream that reports transport errors.
///
/// The previous version mapped every body error to an empty chunk, so a reset
/// or timeout in the middle of a file looked like a clean EOF. Callers could
/// then "complete" a truncated file (only a SHA1 check would catch it). The
/// error is propagated so the task fails and can be resumed from the persisted
/// offset instead of keeping corrupted bytes.
fn into_byte_stream(
  response: reqwest::Response,
) -> impl Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + use<> {
  response.bytes_stream().map(|chunk| {
    chunk.map_err(|error| std::io::Error::new(std::io::ErrorKind::ConnectionReset, error))
  })
}

/// A failure while obtaining the response headers.
enum DownloadRequestError {
  /// The fast-fail deadline elapsed before the headers arrived.
  Timeout,
  /// reqwest could not send the request or the server returned an error status.
  Request(reqwest::Error),
  /// The server answered in a way that makes resuming unsafe, for example by
  /// ignoring a Range request and returning the whole file.
  Protocol(String),
}

impl std::fmt::Display for DownloadRequestError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Timeout => write!(formatter, "Timed out waiting for download response headers"),
      Self::Request(error) => write!(formatter, "{error}"),
      Self::Protocol(reason) => write!(formatter, "{reason}"),
    }
  }
}

/// The retry policy a failed download attempt belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadFailureClass {
  /// The mirror itself did not serve the request. Retrying the same URL only
  /// delays the fallback, so attempts are confined to a short time budget.
  FastFail,
  /// The transfer started but the body was cut short. The bytes already on disk
  /// are contiguous, so a Range request can resume the file.
  Retryable,
}

/// The two failure surfaces of a download attempt.
enum DownloadFailure<'a> {
  /// The request never produced a usable body.
  Request(&'a DownloadRequestError),
  /// The response body stream failed after the transfer had started.
  Body(&'a std::io::Error),
}

/// Classifies a failed download attempt into the retry policy it belongs to.
///
/// Connection and timeout problems mean the mirror is unreachable: the caller
/// gets the error after the bounded fast-fail budget so it can switch source.
/// A reset or short read in the middle of the body is the only failure worth a
/// resume retry.
fn classify_download_failure(failure: DownloadFailure<'_>) -> DownloadFailureClass {
  match failure {
    DownloadFailure::Request(error) => match error {
      // The mirror never answered (or answered with an unusable status): it
      // will not heal by waiting, so fail fast and let the caller switch source.
      DownloadRequestError::Timeout | DownloadRequestError::Protocol(_) => {
        DownloadFailureClass::FastFail
      }
      DownloadRequestError::Request(request_error) => {
        if request_error.is_connect() || request_error.is_timeout() {
          DownloadFailureClass::FastFail
        } else if request_error.is_body() {
          DownloadFailureClass::Retryable
        } else {
          DownloadFailureClass::FastFail
        }
      }
    },
    DownloadFailure::Body(error) => match error.kind() {
      std::io::ErrorKind::ConnectionReset
      | std::io::ErrorKind::ConnectionAborted
      | std::io::ErrorKind::BrokenPipe
      | std::io::ErrorKind::UnexpectedEof
      | std::io::ErrorKind::TimedOut => DownloadFailureClass::Retryable,
      _ => DownloadFailureClass::FastFail,
    },
  }
}

/// Sends a request with the download-specific fast-fail policy.
///
/// `budget` covers every attempt and the backoff between them, so an unreachable
/// mirror cannot hold a download task for the one hour budget used by
/// `utils::web::with_retry`. `attempt` is invoked once per try.
async fn send_with_retry_budget<F, Fut>(
  budget: Duration,
  attempts: usize,
  headers_timeout: Duration,
  mut attempt: F,
) -> Result<reqwest::Response, DownloadRequestError>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<reqwest::Response, DownloadRequestError>>,
{
  let started = Instant::now();
  let attempts = attempts.max(1);
  let mut last_error = None;

  for attempt_index in 1..=attempts {
    let remaining = budget.saturating_sub(started.elapsed());
    if remaining.is_zero() {
      break;
    }

    let result = match tokio::time::timeout(
      remaining.min(headers_timeout.min(DOWNLOAD_HEADERS_TIMEOUT)),
      attempt(),
    )
    .await
    {
      Ok(result) => result,
      Err(_) => Err(DownloadRequestError::Timeout),
    };

    match result {
      Ok(response) => return Ok(response),
      Err(error) => {
        last_error = Some(error);
        if attempt_index == attempts {
          break;
        }
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
          break;
        }
        tokio::time::sleep(DOWNLOAD_FAST_FAIL_BACKOFF.min(remaining)).await;
      }
    }
  }

  Err(last_error.unwrap_or(DownloadRequestError::Timeout))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadParam {
  pub src: Url,
  pub dest: PathBuf,
  pub filename: Option<String>,
  pub sha1: Option<String>,
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

/// Copies everything from the reader to the writer through a fixed
/// DOWNLOAD_BUFFER_SIZE buffer, so one write covers up to 64 KiB instead of the
/// 8 KiB chunks produced by tokio::io::copy.
async fn copy_to_file<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<u64>
where
  R: AsyncRead + Unpin + ?Sized,
  W: AsyncWrite + Unpin + ?Sized,
{
  let mut buffer = vec![0u8; DOWNLOAD_BUFFER_SIZE];
  let mut copied = 0u64;
  loop {
    let read = reader.read(&mut buffer).await?;
    if read == 0 {
      break;
    }
    writer.write_all(&buffer[..read]).await?;
    copied += read as u64;
  }
  Ok(copied)
}

/// Opens the destination file, either truncating it for a fresh download or
/// seeking to the already persisted byte offset when resuming.
async fn prepare_dest_file(dest_path: &Path, current: u64) -> std::io::Result<tokio::fs::File> {
  if current == 0 {
    tokio::fs::File::create(dest_path).await
  } else {
    let mut file = tokio::fs::OpenOptions::new()
      .write(true)
      .open(dest_path)
      .await?;
    file.seek(std::io::SeekFrom::Start(current)).await?;
    Ok(file)
  }
}

/// Validates the downloaded file hash on the blocking pool: hashing a large
/// artifact is pure CPU + disk I/O and must not occupy an async worker thread.
async fn validate_dest_sha1(dest_path: PathBuf, truth: String) -> SJMCLResult<()> {
  tokio::task::spawn_blocking(move || validate_sha1(dest_path, truth))
    .await
    .map_err(|e| SJMCLError(format!("SHA1 validation task failed: {e}")))?
}

/// Copies one response body into `dest_path`, resuming at `current`.
///
/// When a task handle is present the body goes through `ProgressStream` so
/// progress reporting, pause/resume and cancellation keep working; tests pass
/// `None` to drive the same copy path without an `AppHandle`.
async fn write_response_to_file(
  response: reqwest::Response,
  dest_path: &Path,
  current: u64,
  limiter: Option<Limiter>,
  task_handle: Option<&Arc<RwLock<PTaskHandle>>>,
) -> std::io::Result<()> {
  if let Some(parent) = dest_path.parent() {
    tokio::fs::create_dir_all(parent).await?;
  }

  let stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>> =
    match task_handle {
      Some(handle) => Box::pin(ProgressStream::new(
        into_byte_stream(response),
        handle.clone(),
      )),
      None => Box::pin(into_byte_stream(response)),
    };

  let mut file = prepare_dest_file(dest_path, current).await?;
  let reader = Box::pin(stream.into_async_read());
  if let Some(limiter) = limiter {
    let mut limited = limiter.limit(reader).compat();
    copy_to_file(&mut limited, &mut file).await?;
  } else {
    let mut compatible = reader.compat();
    copy_to_file(&mut compatible, &mut file).await?;
  }
  drop(file);
  Ok(())
}

/// Result of trying one source (see `download_from_source`).
enum SourceResult {
  Success,
  Cancelled,
}

/// Downloads one artifact, trying every candidate source in order.
///
/// The first source keeps the caller's resume offset; later ones restart from
/// zero because a different mirror may not serve the same byte ranges. Each
/// outcome feeds `SourceHealth`, so a mirror that has just failed three times in
/// a row is skipped by the next task instead of costing it another timeout.
/// Reads the launcher config and asks the source layer for alternatives to `src`.
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

async fn download_with_policy(
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

  for (index, source) in sources.iter().enumerate() {
    let host = SourceHealth::host_key(source);
    if source_health().is_disabled(&host) {
      attempts.push(format!("{source} (skipped: marked unhealthy)"));
      continue;
    }

    let offset = if index == 0 { initial_offset } else { 0 };
    let started = Instant::now();
    let headers_timeout = source_health().timeout_for(&host);

    match download_from_source(
      client,
      param,
      source,
      dest_path,
      offset,
      headers_timeout,
      limiter.clone(),
      task_handle.clone(),
      retry_same_source_on_sha1,
      expected_size,
    )
    .await
    {
      Ok(SourceResult::Success) => {
        source_health().record_success(&host, started.elapsed());
        return Ok(());
      }
      Ok(SourceResult::Cancelled) => return Ok(()),
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

  Err(SJMCLError(match last_error {
    Some(detail) => format!(
      "Download failed after trying {} source(s): {detail}. Tried: {}",
      attempts.len(),
      attempts.join(" | ")
    ),
    None => format!(
      "Download failed: every candidate source is currently marked unhealthy ({} attempted)",
      attempts.len()
    ),
  }))
}

/// Runs the transfer against one source URL.
#[allow(clippy::too_many_arguments)]
async fn download_from_source(
  client: &Client,
  param: &DownloadParam,
  source: &Url,
  dest_path: &Path,
  initial_offset: u64,
  headers_timeout: Duration,
  limiter: Option<Limiter>,
  task_handle: Option<Arc<RwLock<PTaskHandle>>>,
  retry_same_source_on_sha1: bool,
  expected_size: Option<u64>,
) -> SJMCLResult<SourceResult> {
  let mut offset = initial_offset;
  let mut resume_retries = 0usize;
  let mut sha1_retries = 0usize;
  let mut first_attempt = true;

  loop {
    if task_handle
      .as_ref()
      .is_some_and(|handle| handle.read().unwrap().status().is_cancelled())
    {
      let _ = tokio::fs::remove_file(dest_path).await;
      return Ok(SourceResult::Cancelled);
    }

    let response =
      match DownloadTask::send_request(client, offset, source, param, headers_timeout).await {
        Ok(response) => response,
        Err(error) => {
          let class = classify_download_failure(DownloadFailure::Request(&error));
          log::warn!("Download request failed ({class:?}) for {source}: {error}");
          return Err(SJMCLError(error.to_string()));
        }
      };

    // Reject a wrong-sized body before paying for the transfer. A mirror that
    // answers 200 with a different object would otherwise waste the whole
    // download and only fail SHA-1 validation afterwards.
    if offset == 0
      && let (Some(expected), Some(actual)) = (expected_size, response.content_length())
      && expected != actual
    {
      return Err(SJMCLError(format!(
        "Size mismatch: expected {expected} bytes, the source announced {actual}"
      )));
    }

    let total_progress = if offset == 0 {
      response.content_length().map_or(-1, |length| length as i64)
    } else {
      -1
    };

    if let Some(handle) = task_handle.as_ref() {
      let mut handle = handle.write().unwrap();
      handle.set_total(total_progress);
      if first_attempt {
        handle.mark_started();
      }
    }
    first_attempt = false;

    if let Err(error) = write_response_to_file(
      response,
      dest_path,
      offset,
      limiter.clone(),
      task_handle.as_ref(),
    )
    .await
    {
      if task_handle
        .as_ref()
        .is_some_and(|handle| handle.read().unwrap().status().is_cancelled())
      {
        let _ = tokio::fs::remove_file(dest_path).await;
        return Ok(SourceResult::Cancelled);
      }

      if classify_download_failure(DownloadFailure::Body(&error)) == DownloadFailureClass::Retryable
        && resume_retries < DOWNLOAD_RESUME_RETRIES
      {
        resume_retries += 1;
        offset = tokio::fs::metadata(dest_path)
          .await
          .map_or(0, |metadata| metadata.len());
        log::warn!(
          "Download body interrupted ({error}), resuming at byte {offset} ({resume_retries}/{DOWNLOAD_RESUME_RETRIES})"
        );
        continue;
      }

      return Err(SJMCLError(format!("Download failed: {error}")));
    }

    if task_handle
      .as_ref()
      .is_some_and(|handle| handle.read().unwrap().status().is_cancelled())
    {
      tokio::fs::remove_file(dest_path).await?;
      return Ok(SourceResult::Cancelled);
    }

    let validation = match &param.sha1 {
      // Validate the file that was actually written (dest_path), not the raw
      // manifest destination, which may be relative to the cache dir.
      Some(truth) => validate_dest_sha1(dest_path.to_path_buf(), truth.clone()).await,
      None => Ok(()),
    };

    if let Err(error) = validation {
      // Never keep an unverified artifact: deleting it first guarantees the next
      // attempt starts from scratch instead of resuming a truncated file.
      tokio::fs::remove_file(dest_path)
        .await
        .map_err(|remove_error| {
          SJMCLError(format!(
            "Failed to remove the invalid download: {remove_error}"
          ))
        })?;

      // With alternative sources available, a corrupted body is a source
      // problem: let the failover loop pick a different one instead of paying
      // for the same mirror again.
      if !retry_same_source_on_sha1 || sha1_retries == DOWNLOAD_SHA1_RETRIES {
        return Err(SJMCLError(format!("SHA1 validation failed: {}", error.0)));
      }
      sha1_retries += 1;
      offset = 0;
      log::warn!(
        "SHA1 validation failed ({}), retrying from scratch ({sha1_retries}/{DOWNLOAD_SHA1_RETRIES})",
        error.0
      );
      continue;
    }

    return Ok(SourceResult::Success);
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
      // No manifest size is available on this path yet; the size check stays off
      // instead of guessing one.
      expected_size: None,
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
      // No manifest size is available on this path yet; the size check stays off
      // instead of guessing one.
      expected_size: None,
      param: param.clone(),
      dest_path: cache_dir.clone().join(param.dest.clone()),
      report_interval,
    }
  }

  async fn send_request(
    client: &Client,
    current: u64,
    source: &Url,
    param: &DownloadParam,
    headers_timeout: Duration,
  ) -> Result<reqwest::Response, DownloadRequestError> {
    let response = send_with_retry_budget(
      DOWNLOAD_FAST_FAIL_BUDGET,
      DOWNLOAD_FAST_FAIL_ATTEMPTS,
      headers_timeout,
      || {
        let mut request = if current == 0 {
          client.get(source.clone())
        } else {
          client
            .get(source.clone())
            .header(RANGE, format!("bytes={current}-"))
        };

        // Never negotiate a compressed body: it hides the real byte offsets from
        // Range/resume handling and adds decompression work. PCL2-CE sends the same
        // header for its Range downloads.
        request = request.header(ACCEPT_ENCODING, "identity");

        // add api key header for CurseForge download urls (#1679)
        // ref: https://blog.curseforge.com/introducing-api-key-authentication-for-curseforge-file-downloads
        if is_curseforge_authenticated_url(&param.src) {
          request = request.header("x-api-key", CURSEFORGE_API_KEY.as_str());
        }

        async move {
          let response = request
            .send()
            .await
            .map_err(DownloadRequestError::Request)?;
          response
            .error_for_status()
            .map_err(DownloadRequestError::Request)
        }
      },
    )
    .await?;

    ensure_resumed_response(current, response.status())
      .map_err(|error| DownloadRequestError::Protocol(error.0))?;
    Ok(response)
  }

  async fn future_impl(
    self,
    app_handle: AppHandle,
    limiter: Option<Limiter>,
  ) -> SJMCLResult<(
    impl Future<Output = SJMCLResult<()>> + Send,
    Arc<RwLock<PTaskHandle>>,
  )> {
    let current = resume_offset(self.p_handle.desc.current);
    let alt_srcs = self.alt_srcs.clone();
    let expected_size = self.expected_size;
    let handle = Arc::new(RwLock::new(self.p_handle));
    let task_handle = handle.clone();
    let param = self.param.clone();
    Ok((
      async move {
        let client = download_client(&app_handle);
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
mod tests {
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

  fn unique_temp_path(tag: &str) -> PathBuf {
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
    };

    let started = std::time::Instant::now();
    let result =
      DownloadTask::send_request(&client, 0, &param.src, &param, Duration::from_secs(30)).await;
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

  fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
  }

  /// One canned HTTP/1.1 reply, served on its own TCP connection.
  enum FakeReply {
    /// Advertise the full body length but write only `sent` bytes before
    /// closing: the client sees a short read / connection reset.
    TruncatedBody { body: Vec<u8>, sent: usize },
    /// No Content-Length: the connection close delimits the body, so a short
    /// write is a clean EOF (the silent truncation observed on the mirror).
    BodyEndedByClose { body: Vec<u8> },
    /// `206 Partial Content` with the tail starting at `offset`.
    RangeTail { body: Vec<u8>, offset: usize },
    /// An error status with an empty body, e.g. a dead mirror path.
    Status { code: u16, reason: &'static str },
  }

  struct FakeHttpServer {
    url: String,
    request_heads: Arc<Mutex<Vec<String>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
  }

  impl FakeHttpServer {
    fn spawn(replies: Vec<FakeReply>) -> Self {
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
  async fn every_source_failing_reports_what_was_tried() {
    let first = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 500,
      reason: "Internal Server Error",
    }]);
    let second = FakeHttpServer::spawn(vec![FakeReply::Status {
      code: 404,
      reason: "Not Found",
    }]);
    let client = download_client_builder().no_proxy().build().unwrap();
    let dest = unique_temp_path("all-sources-fail");
    let param = DownloadParam {
      src: Url::parse(&first.url).unwrap(),
      dest: PathBuf::from("file.bin"),
      filename: None,
      sha1: None,
    };
    let alt = [Url::parse(&second.url).unwrap()];

    let error = download_with_policy(&client, &param, &alt[..], None, &dest, 0, None, None)
      .await
      .expect_err("both sources fail, so the task must fail");

    assert!(error.0.contains("after trying 2 source(s)"), "{}", error.0);
    assert!(error.0.contains(&first.url), "{}", error.0);
    assert!(error.0.contains(&second.url), "{}", error.0);
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
}
