//! Behaviour-compatible sequential download engine.
//!
//! This is the pre-abstraction transfer logic moved behind
//! [`DownloadEngine`]: one response body is copied to the destination file at a
//! time, resuming with `Range` after a truncated body and re-downloading from
//! scratch when SHA-1 validation fails. The surrounding failover driver
//! (`tasks::download::download_with_policy`) still decides which URL to try and
//! how a failure affects source health; this module only knows "how to transfer
//! one source".

use std::future::Future;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::time::{Duration, Instant};

use async_speed_limit::Limiter;
use futures::Stream;
use futures::StreamExt;
use futures::stream::TryStreamExt;
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use tauri::Url;
use tauri_plugin_http::reqwest;
use tauri_plugin_http::reqwest::header::{ACCEPT_ENCODING, RANGE};
use tauri_plugin_http::reqwest::{Client, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tokio_util::bytes;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::resource::helpers::curseforge::misc::{
  CURSEFORGE_API_KEY, is_curseforge_authenticated_url,
};
use crate::utils::fs::validate_sha1;

use super::{DownloadEngine, FetchOutcome, FetchPlan, ProgressSink};

/// Buffer size used when copying the response stream into the destination file.
///
/// The previous implementation used tokio::io::copy, whose internal buffer is
/// only 8 KiB: every 8 KiB of payload caused one socket read, one file write
/// and one progress-stream poll. 64 KiB matches the sequential download buffer
/// used by PCL2 (80 KiB) and PCL2-CE (64 KiB) and divides the number of
/// syscalls / stream polls per megabyte by 8.
pub(crate) const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;

/// Maximum time to wait for the response headers (connect + TLS + redirects +
/// time-to-first-byte). `send()` resolves as soon as the headers arrive, so this
/// deadline never covers the body itself.
const DOWNLOAD_HEADERS_TIMEOUT: Duration = Duration::from_secs(30);

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
pub(crate) const DOWNLOAD_SHA1_RETRIES: usize = 2;

/// Sequential transfer strategy: one body at a time, with the pre-engine
/// retry and resume policies.
///
/// The fields are the per-attempt network policy that is not part of
/// [`FetchPlan`]. [`DownloadEngine::fetch`] receives no other arguments, and
/// the failover driver already had to compute the header deadline from
/// [`crate::tasks::source_health::SourceHealth`], so the driver configures one
/// engine per source attempt instead of widening the plan.
pub struct SequentialEngine {
  limiter: Option<Limiter>,
  headers_timeout: Duration,
  retry_sha1_on_same_source: bool,
}

impl Default for SequentialEngine {
  fn default() -> Self {
    Self {
      limiter: None,
      headers_timeout: DOWNLOAD_HEADERS_TIMEOUT,
      // A single source is the historical "retry the same URL" behaviour,
      // which is what rescues truncation on an otherwise healthy official
      // source.
      retry_sha1_on_same_source: true,
    }
  }
}

impl SequentialEngine {
  pub fn new() -> Self {
    Self::default()
  }

  /// Applies the launcher's global download rate limiter.
  #[must_use]
  pub fn with_limiter(mut self, limiter: Option<Limiter>) -> Self {
    self.limiter = limiter;
    self
  }

  /// Sets the per-source deadline for receiving response headers, as chosen
  /// by `SourceHealth` from the measured latency of the mirror.
  #[must_use]
  pub fn with_headers_timeout(mut self, headers_timeout: Duration) -> Self {
    self.headers_timeout = headers_timeout;
    self
  }

  /// Whether a SHA-1 failure may retry the same URL. When the driver has more
  /// sources left, a corrupted body is treated as a source problem and the
  /// error is returned immediately so the next mirror is tried instead.
  #[must_use]
  pub fn with_sha1_retry_on_same_source(mut self, retry: bool) -> Self {
    self.retry_sha1_on_same_source = retry;
    self
  }

  async fn fetch_inner(
    &self,
    client: &Client,
    plan: &FetchPlan,
    sink: &mut dyn ProgressSink,
  ) -> SJMCLResult<FetchOutcome> {
    let Some(source) = plan.sources.first() else {
      // Defensive: the failover driver always selects a source, but an empty
      // plan must fail cleanly instead of panicking or touching the file.
      return Err(SJMCLError(
        "Download failed: the fetch plan contains no source URL".to_string(),
      ));
    };

    let mut offset = plan.resume_offset;
    let mut resume_retries = 0usize;
    let mut sha1_retries = 0usize;
    let mut first_attempt = true;

    loop {
      // A pause can arrive while the previous attempt is backing off, or while a
      // failed source hands over to the next one, so the retry path parks here
      // instead of trusting only the state captured when the fetch started
      // (#2079). Cancellation is checked right after the park, so a cancelled
      // task leaves the loop instead of issuing another request.
      sink.wait_if_paused().await;
      if sink.is_cancelled() {
        let _ = tokio::fs::remove_file(&plan.dest).await;
        return Ok(FetchOutcome::Cancelled);
      }

      let response = match Self::send_request(client, offset, source, self.headers_timeout).await {
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
        && let (Some(expected), Some(actual)) = (plan.size, response.content_length())
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

      sink.on_total(total_progress);
      if first_attempt {
        sink.on_started();
      }
      first_attempt = false;

      match write_response(response, &plan.dest, offset, self.limiter.as_ref(), sink).await {
        Ok(CopyOutcome::Completed) => sink.on_complete(),
        Ok(CopyOutcome::Cancelled) => {
          let _ = tokio::fs::remove_file(&plan.dest).await;
          return Ok(FetchOutcome::Cancelled);
        }
        Err(error) => {
          if sink.is_cancelled() {
            let _ = tokio::fs::remove_file(&plan.dest).await;
            return Ok(FetchOutcome::Cancelled);
          }

          if classify_download_failure(DownloadFailure::Body(&error))
            == DownloadFailureClass::Retryable
            && resume_retries < DOWNLOAD_RESUME_RETRIES
          {
            resume_retries += 1;
            offset = tokio::fs::metadata(&plan.dest)
              .await
              .map_or(0, |metadata| metadata.len());
            log::warn!(
              "Download body interrupted ({error}), resuming at byte {offset} ({resume_retries}/{DOWNLOAD_RESUME_RETRIES})"
            );
            continue;
          }

          return Err(SJMCLError(format!("Download failed: {error}")));
        }
      }

      if sink.is_cancelled() {
        tokio::fs::remove_file(&plan.dest).await?;
        return Ok(FetchOutcome::Cancelled);
      }

      let validation = match &plan.sha1 {
        // Validate the file that was actually written (dest), not the raw
        // manifest destination, which may be relative to the cache dir.
        Some(truth) => validate_dest_sha1(plan.dest.clone(), truth.clone()).await,
        None => Ok(()),
      };

      if let Err(error) = validation {
        // Never keep an unverified artifact: deleting it first guarantees the
        // next attempt starts from scratch instead of resuming a truncated
        // file.
        tokio::fs::remove_file(&plan.dest)
          .await
          .map_err(|remove_error| {
            SJMCLError(format!(
              "Failed to remove the invalid download: {remove_error}"
            ))
          })?;

        // With alternative sources available, a corrupted body is a source
        // problem: let the failover loop pick a different one instead of
        // paying for the same mirror again.
        if !self.retry_sha1_on_same_source || sha1_retries == DOWNLOAD_SHA1_RETRIES {
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

      return Ok(FetchOutcome::Completed);
    }
  }

  /// Sends the request for one attempt and rejects responses that cannot be
  /// used (error status, Range ignored while resuming).
  ///
  /// The CurseForge API key is attached based on the URL being fetched. The
  /// failover layer only rewrites official Mojang hosts, so an alternative URL
  /// never turns a non-CurseForge primary into a CurseForge host or vice versa;
  /// checking the actual source therefore matches the old primary-URL check for
  /// every reachable input.
  pub(crate) async fn send_request(
    client: &Client,
    current: u64,
    source: &Url,
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
        if is_curseforge_authenticated_url(source) {
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
}

impl DownloadEngine for SequentialEngine {
  fn fetch<'a>(
    &'a self,
    client: &'a Client,
    plan: &'a FetchPlan,
    sink: &'a mut dyn ProgressSink,
  ) -> Pin<Box<dyn Future<Output = SJMCLResult<FetchOutcome>> + Send + 'a>> {
    Box::pin(self.fetch_inner(client, plan, sink))
  }
}

/// A failure while obtaining the response headers.
pub(crate) enum DownloadRequestError {
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
pub(crate) enum DownloadFailureClass {
  /// The mirror itself did not serve the request. Retrying the same URL only
  /// delays the fallback, so attempts are confined to a short time budget.
  FastFail,
  /// The transfer started but the body was cut short. The bytes already on disk
  /// are contiguous, so a Range request can resume the file.
  Retryable,
}

/// The two failure surfaces of a download attempt.
pub(crate) enum DownloadFailure<'a> {
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
pub(crate) fn classify_download_failure(failure: DownloadFailure<'_>) -> DownloadFailureClass {
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
pub(crate) async fn send_with_retry_budget<F, Fut>(
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

/// A resumed download (`Range: bytes=N-`) must be answered with `206 Partial
/// Content`. If a server or proxy ignores the header and returns `200 OK` with
/// the whole file, appending it after the persisted prefix would corrupt the
/// artifact, so fail explicitly instead of silently writing bad bytes.
pub(crate) fn ensure_resumed_response(current: u64, status: StatusCode) -> SJMCLResult<()> {
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
pub(crate) fn into_byte_stream(
  response: reqwest::Response,
) -> impl Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + use<> {
  response.bytes_stream().map(|chunk| {
    chunk.map_err(|error| std::io::Error::new(std::io::ErrorKind::ConnectionReset, error))
  })
}

/// Copies everything from the reader to the writer through a fixed
/// DOWNLOAD_BUFFER_SIZE buffer, so one write covers up to 64 KiB instead of the
/// 8 KiB chunks produced by tokio::io::copy.
///
/// Kept for the buffer-size regression test: the production path uses
/// [`copy_to_file_with_sink`] with a progress sink, which is the same loop.
#[cfg(test)]
pub(crate) async fn copy_to_file<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<u64>
where
  R: AsyncRead + Unpin + ?Sized,
  W: AsyncWrite + Unpin + ?Sized,
{
  let mut sink = super::NullProgressSink::new();
  let (copied, _) = copy_to_file_with_sink(reader, writer, &mut sink).await?;
  Ok(copied)
}

/// Whether a copy loop reached the end of the body or stopped for cancellation.
enum CopyOutcome {
  Completed,
  Cancelled,
}

/// Copies the body through `sink`, which also decides when a paused or
/// cancelled task must stop pulling bytes.
async fn copy_to_file_with_sink<R, W>(
  reader: &mut R,
  writer: &mut W,
  sink: &mut dyn ProgressSink,
) -> std::io::Result<(u64, CopyOutcome)>
where
  R: AsyncRead + Unpin + ?Sized,
  W: AsyncWrite + Unpin + ?Sized,
{
  let mut buffer = vec![0u8; DOWNLOAD_BUFFER_SIZE];
  let mut copied = 0u64;
  loop {
    sink.wait_if_paused().await;
    if sink.is_cancelled() {
      return Ok((copied, CopyOutcome::Cancelled));
    }

    let read = reader.read(&mut buffer).await?;
    if read == 0 {
      break;
    }
    writer.write_all(&buffer[..read]).await?;
    copied += read as u64;
    sink.on_bytes(read as u64);
  }
  Ok((copied, CopyOutcome::Completed))
}

/// Opens the destination file, either truncating it for a fresh download or
/// seeking to the already persisted byte offset when resuming.
pub(crate) async fn prepare_dest_file(
  dest_path: &Path,
  current: u64,
) -> std::io::Result<tokio::fs::File> {
  if current == 0 {
    tokio::fs::File::create(dest_path).await
  } else {
    let mut file = tokio::fs::OpenOptions::new()
      .write(true)
      .open(dest_path)
      .await?;
    file.seek(SeekFrom::Start(current)).await?;
    Ok(file)
  }
}

/// Copies one response body into `dest_path`, resuming at `current`.
///
/// The body goes through `sink` so progress reporting, pause/resume and
/// cancellation keep working; tests pass [`NullProgressSink`] to drive the same
/// copy path without an `AppHandle`.
async fn write_response(
  response: reqwest::Response,
  dest_path: &Path,
  current: u64,
  limiter: Option<&Limiter>,
  sink: &mut dyn ProgressSink,
) -> std::io::Result<CopyOutcome> {
  if let Some(parent) = dest_path.parent() {
    tokio::fs::create_dir_all(parent).await?;
  }

  let stream = into_byte_stream(response);
  let mut file = prepare_dest_file(dest_path, current).await?;
  let reader = stream.into_async_read();
  let outcome = if let Some(limiter) = limiter {
    let mut limited = limiter.clone().limit(reader).compat();
    copy_to_file_with_sink(&mut limited, &mut file, sink)
      .await?
      .1
  } else {
    let mut compatible = reader.compat();
    copy_to_file_with_sink(&mut compatible, &mut file, sink)
      .await?
      .1
  };
  drop(file);
  Ok(outcome)
}

/// Validates the downloaded file hash on the blocking pool: hashing a large
/// artifact is pure CPU + disk I/O and must not occupy an async worker thread.
pub(crate) async fn validate_dest_sha1(
  dest_path: std::path::PathBuf,
  truth: String,
) -> SJMCLResult<()> {
  tokio::task::spawn_blocking(move || validate_sha1(dest_path, truth))
    .await
    .map_err(|e| SJMCLError(format!("SHA1 validation task failed: {e}")))?
}
