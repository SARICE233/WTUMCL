use async_speed_limit::Limiter;
use futures::StreamExt;
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::error::Error;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;
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
use crate::tasks::streams::ProgressStream;
use crate::tasks::streams::desc::{PDesc, PStatus};
use crate::tasks::streams::reporter::Reporter;
use crate::tasks::*;
use crate::utils::fs::validate_sha1;
use crate::utils::web::with_retry;

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

/// Waits for the response headers with a bounded deadline.
///
/// reqwest has no header-only timeout; wrapping `send()` is enough because it
/// resolves as soon as the headers are in. The body is not covered here (that is
/// `DOWNLOAD_READ_TIMEOUT`), so a legitimately long transfer is unaffected.
async fn send_with_header_deadline<F>(deadline: Duration, send: F) -> SJMCLResult<reqwest::Response>
where
  F: Future<Output = Result<reqwest::Response, reqwest_middleware::Error>>,
{
  tokio::time::timeout(deadline, send)
    .await
    .map_err(|_| SJMCLError("Timed out waiting for download response headers".to_string()))?
    .map_err(|e| SJMCLError(format!("{:?}", e.source())))
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
  report_interval: Duration,
}

/// Reads only the progress report interval from the launcher config instead of
/// cloning the whole config for every scheduled download task.
fn download_cache_dir(app_handle: &AppHandle) -> PathBuf {
  let config = app_handle.state::<Mutex<LauncherConfig>>();
  let cache_dir = config.lock().unwrap().download.cache.directory.clone();
  cache_dir
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
      param: param.clone(),
      dest_path: cache_dir.clone().join(param.dest.clone()),
      report_interval,
    }
  }

  async fn send_request(
    app_handle: &AppHandle,
    current: u64,
    param: &DownloadParam,
  ) -> SJMCLResult<reqwest::Response> {
    let client = with_retry(download_client(app_handle).clone());
    let mut request = if current == 0 {
      client.get(param.src.clone())
    } else {
      client
        .get(param.src.clone())
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

    // No client-level total deadline is set (a file may legitimately take
    // hours), so bound only the time until the response headers arrive. A
    // server that accepts the connection but never answers then fails instead
    // of hanging forever; the body is guarded by DOWNLOAD_READ_TIMEOUT.
    let response = send_with_header_deadline(DOWNLOAD_HEADERS_TIMEOUT, request.send()).await?;

    let response = response
      .error_for_status()
      .map_err(|e| SJMCLError(format!("{:?}", e.source())))?;

    ensure_resumed_response(current, response.status())?;
    Ok(response)
  }

  async fn create_resp_stream(
    app_handle: &AppHandle,
    current: u64,
    param: &DownloadParam,
  ) -> SJMCLResult<(
    impl Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + use<>,
    i64,
  )> {
    let resp = Self::send_request(app_handle, current, param).await?;
    let total_progress = if current == 0 {
      resp.content_length().map_or(-1, |length| length as i64)
    } else {
      -1
    };
    Ok((into_byte_stream(resp), total_progress))
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
    let handle = Arc::new(RwLock::new(self.p_handle));
    let task_handle = handle.clone();
    let param = self.param.clone();
    Ok((
      async move {
        let (resp, total_progress) = Self::create_resp_stream(&app_handle, current, &param).await?;
        let stream = ProgressStream::new(resp, task_handle.clone());
        tokio::fs::create_dir_all(&self.dest_path.parent().unwrap()).await?;
        let mut file = prepare_dest_file(&self.dest_path, current).await?;
        {
          let mut task_handle = task_handle.write().unwrap();
          task_handle.set_total(total_progress);
          task_handle.mark_started();
        }
        let mut reader = stream.into_async_read();
        if let Some(lim) = limiter {
          let mut limited = lim.limit(reader).compat();
          copy_to_file(&mut limited, &mut file).await?;
        } else {
          let mut compat = reader.compat();
          copy_to_file(&mut compat, &mut file).await?;
        }
        drop(file);
        if task_handle.read().unwrap().status().is_cancelled() {
          tokio::fs::remove_file(&self.dest_path).await?;
          Ok(())
        } else {
          match param.sha1 {
            // Validate the file that was actually written (dest_path), not the
            // raw manifest destination, which may be relative to the cache dir.
            Some(truth) => validate_dest_sha1(self.dest_path.clone(), truth).await,
            None => Ok(()),
          }
        }
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

    let client = with_retry(download_client_builder().no_proxy().build().unwrap());
    let started = std::time::Instant::now();
    let result =
      send_with_header_deadline(Duration::from_millis(150), client.get(&url).send()).await;

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
}
