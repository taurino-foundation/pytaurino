use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::sync_channel,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use taunicorn::{Connection, ReceiveResult};
use tokio::{
    runtime::Builder,
    sync::{
        mpsc::{self, Sender},
        watch,
    },
    time::timeout,
};

use crate::config::{IpcClientConfig, IpcSecurity};

const CHANNEL_CAPACITY: usize = 256;

// ------------------------------------------------------------
// Messages
// ------------------------------------------------------------

#[derive(Debug)]
pub enum IPCMessage {
    Send(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum IpcEvent {
    Message(Vec<u8>),
    Disconnected(String),
}

// ------------------------------------------------------------
// Shared client state
// ------------------------------------------------------------

struct Inner {
    sender: Sender<IPCMessage>,
    shutdown: watch::Sender<bool>,
    join: Mutex<Option<JoinHandle<Result<()>>>>,
    stopping: AtomicBool,
    max_message_size: usize,
}

#[derive(Clone)]
pub struct IpcClient {
    inner: Arc<Inner>,
}

// ------------------------------------------------------------
// Public client
// ------------------------------------------------------------

impl IpcClient {
    /// Starts a dedicated Tokio thread and connects to Python.
    ///
    /// Returns only after the transport connection succeeds.
    /// on_event executes on the IPC worker thread.
    pub fn new<F>(config: &IpcClientConfig, on_event: F) -> Result<Self>
    where
        F: Fn(IpcEvent) + Send + Sync + 'static,
    {
        config.validate()?;

        // Authentication needs the matching Taunicorn
        // security handshake and must not be bypassed.
        ensure!(
            matches!(config.security, IpcSecurity::None),
            "Authenticated IPC is not implemented yet"
        );

        let endpoint = config.endpoint.clone();
        let connect_timeout = Duration::from_secs_f64(config.connect_timeout);

        let operation_timeout = Duration::from_secs_f64(config.operation_timeout);

        let max_message_size = usize::try_from(config.max_message_size)?;

        let on_event: Arc<dyn Fn(IpcEvent) + Send + Sync> = Arc::new(on_event);

        let (sender, receiver) = mpsc::channel::<IPCMessage>(CHANNEL_CAPACITY);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Synchronous startup acknowledgement.
        let (ready_tx, ready_rx) = sync_channel::<Result<(), String>>(1);

        let worker = thread::Builder::new()
            .name("taurino-ipc".into())
            .spawn(move || -> Result<()> {
                let rt = match Builder::new_current_thread().enable_all().build() {
                    Ok(rt) => rt,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return Err(error.into());
                    }
                };

                let connection =
                    match rt.block_on(Connection::connect_timeout(endpoint, connect_timeout)) {
                        Ok(connection) => connection,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return Err(error.into());
                        }
                    };

                // Python transport is connected.
                let _ = ready_tx.send(Ok(()));

                rt.block_on(run_worker(
                    connection,
                    receiver,
                    shutdown_rx,
                    on_event,
                    max_message_size,
                    operation_timeout,
                ))
            })
            .context("Failed to spawn IPC worker")?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}

            Ok(Err(message)) => {
                let _ = worker.join();
                bail!("IPC startup failed: {message}");
            }

            Err(error) => {
                let _ = worker.join();
                return Err(error.into());
            }
        }

        Ok(Self {
            inner: Arc::new(Inner {
                sender,
                shutdown: shutdown_tx,
                join: Mutex::new(Some(worker)),
                stopping: AtomicBool::new(false),
                max_message_size,
            }),
        })
    }

    /// Queue an already serialized MessagePack payload.
    ///
    /// This method is nonblocking and can be called
    /// from the Tao GUI thread.
    pub fn send(&self, payload: Vec<u8>) -> Result<()> {
        ensure!(
            !self.inner.stopping.load(Ordering::Acquire),
            "IPC client is stopping"
        );

        ensure!(
            !payload.is_empty() && payload.len() <= self.inner.max_message_size,
            "Invalid IPC payload size"
        );

        self.inner
            .sender
            .try_send(IPCMessage::Send(payload))
            .context("IPC outgoing queue is full or closed")?;

        Ok(())
    }

    pub fn is_stopping(&self) -> bool {
        self.inner.stopping.load(Ordering::Acquire)
    }

    /// Nonblocking shutdown request.
    pub fn request_shutdown(&self) {
        self.inner.stopping.store(true, Ordering::Release);
        let _ = self.inner.shutdown.send(true);
    }

    /// Request shutdown and wait until the worker finishes.
    ///
    /// Call after exiting the GUI event loop, not from
    /// an active Tao event callback.
    pub fn shutdown(&self) -> Result<()> {
        self.request_shutdown();

        let mut guard = self
            .inner
            .join
            .lock()
            .map_err(|_| anyhow!("IPC worker mutex poisoned"))?;

        if let Some(handle) = guard.as_ref() {
            ensure!(
                handle.thread().id() != thread::current().id(),
                "Cannot join IPC worker from itself"
            );
        }

        let handle = guard.take();
        drop(guard);

        if let Some(handle) = handle {
            handle
                .join()
                .map_err(|_| anyhow!("IPC worker panicked"))??;
        }

        Ok(())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = self.shutdown.send(true);

        // Do not join in Drop: never block the GUI thread.
        // Dropping the JoinHandle detaches the worker.
        // The worker should exit upon the shutdown signal.
    }
}

// ------------------------------------------------------------
// Tokio worker
// ------------------------------------------------------------

async fn run_worker(
    connection: Connection,
    mut receiver: mpsc::Receiver<IPCMessage>,
    mut shutdown: watch::Receiver<bool>,
    on_event: Arc<dyn Fn(IpcEvent) + Send + Sync>,
    max_message_size: usize,
    operation_timeout: Duration,
) -> Result<()> {
    let connection = Arc::new(connection);

    let reader_connection = Arc::clone(&connection);
    let reader_events = Arc::clone(&on_event);

    // Dedicated reader prevents cancellation in the middle
    // of a MessagePack frame when new outgoing data arrives.
    let mut reader = tokio::spawn(async move {
        loop {
            let payload =
                match read_frame(&reader_connection, max_message_size, operation_timeout).await? {
                    Some(payload) => payload,
                    None => bail!("Python IPC server disconnected"),
                };

            reader_events(IpcEvent::Message(payload));
        }

        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    let result: Result<()> = loop {
        tokio::select! {
            biased;

            change = shutdown.changed() => {
                if change.is_err() || *shutdown.borrow() {
                    break Ok(());
                }
            }

            reader_result = &mut reader => {
                break match reader_result {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(error)) => Err(error),
                    Err(error) => Err(error.into()),
                };
            }

            message = receiver.recv() => {
                match message {
                    Some(IPCMessage::Send(payload)) => {
                        if let Err(error) = write_frame(
                            &connection,
                            &payload,
                            operation_timeout,
                        ).await {
                            break Err(error);
                        }
                    }

                    None => break Ok(()),
                }
            }
        }
    };

    reader.abort();
    let _ = connection.close().await;

    if let Err(error) = &result {
        on_event(IpcEvent::Disconnected(format!("{error:#}")));
    }

    result
}

// ------------------------------------------------------------
// Message framing
// ------------------------------------------------------------

/// Read an exact number of bytes from Taunicorn.
async fn read_exact(connection: &Connection, mut buffer: &mut [u8]) -> Result<()> {
    while !buffer.is_empty() {
        match connection.receive(buffer).await? {
            ReceiveResult::Data(n) if n > 0 => {
                let (_, rest) = buffer.split_at_mut(n);
                buffer = rest;
            }

            ReceiveResult::EndOfStream => {
                bail!("Unexpected IPC end of stream");
            }

            _ => bail!("IPC receive made no progress"),
        }
    }

    Ok(())
}

/// Python protocol:
/// 4-byte big-endian unsigned size + MessagePack payload.
async fn read_frame(
    connection: &Connection,
    max_message_size: usize,
    operation_timeout: Duration,
) -> Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];

    // Wait for the first byte without an idle timeout.
    match connection.receive(&mut header[..1]).await? {
        ReceiveResult::EndOfStream => return Ok(None),
        ReceiveResult::Data(1) => {}
        _ => bail!("Invalid IPC frame header"),
    }

    let payload = timeout(operation_timeout, async {
        read_exact(connection, &mut header[1..]).await?;

        let size = u32::from_be_bytes(header) as usize;

        ensure!(
            size > 0 && size <= max_message_size,
            "IPC frame exceeds allowed size"
        );

        let mut payload = vec![0u8; size];
        read_exact(connection, &mut payload).await?;

        Ok::<Vec<u8>, anyhow::Error>(payload)
    })
    .await
    .context("IPC receive timed out")??;

    Ok(Some(payload))
}

/// Send exactly one framed MessagePack payload.
async fn write_frame(
    connection: &Connection,
    payload: &[u8],
    operation_timeout: Duration,
) -> Result<()> {
    let size = u32::try_from(payload.len())?;

    let mut frame = Vec::with_capacity(4 + payload.len());

    frame.extend_from_slice(&size.to_be_bytes());
    frame.extend_from_slice(payload);

    timeout(operation_timeout, connection.send(&frame))
        .await
        .context("IPC send timed out")??;

    Ok(())
}
