//! The connection a `subscribe` holds, played by the trace (docs/streams.md section 4). The trace is the server: `open`,
//! `drop`, `refuse` and `message` steps move the machine below, `advance` fires a due retry, `hide` and `show` close and reopen
//! the connection. Its shape, its waits and its five words are the page adapter's (`__cui_stream_new`);
//! `tests/stream_corpus.json` is checked against both.

use cancho_ui_ir::stream::{BACKOFF_MS, MESSAGE_MAX};

use crate::eval::Fault;
use crate::machine::{Machine, Outcome, StepEffects};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStatus {
    Connecting,
    Open,
    Retrying,
    Paused,
    Failed,
}

impl StreamStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamStatus::Connecting => "connecting",
            StreamStatus::Open => "open",
            StreamStatus::Retrying => "retrying",
            StreamStatus::Paused => "paused",
            StreamStatus::Failed => "failed",
        }
    }
}

/// The state of the one stream a component may declare.
#[derive(Debug, Clone)]
pub struct StreamState {
    /// The handlers of the message event and of the optional status event.
    pub message: usize,
    pub status_event: Option<usize>,
    pub status: StreamStatus,
    /// Which wait of `BACKOFF_MS` the next drop uses; an `open` sets it back to the first.
    pub step: usize,
    /// The drops since the connection last opened (0 while it is open or has never dropped).
    pub attempt: u32,
    /// The fake-clock time a `retrying` stream connects again.
    pub retry_due: Option<f64>,
    pub last_event_id: Option<String>,
}

impl StreamState {
    pub fn new(message: usize, status_event: Option<usize>) -> StreamState {
        StreamState {
            message,
            status_event,
            status: StreamStatus::Connecting,
            step: 0,
            attempt: 0,
            retry_due: None,
            last_event_id: None,
        }
    }
}

/// The JavaScript `.length` of `text`: UTF-16 code units, which is what the page counts a message in.
fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

impl Machine<'_> {
    /// Move the status and run the status event with its word, if the component declared one.
    fn set_stream_status(&mut self, to: StreamStatus, fx: &mut StepEffects) -> Result<(), Fault> {
        let Some(s) = self.stream.as_mut() else {
            return Ok(());
        };
        s.status = to;
        if let Some(h) = s.status_event {
            self.dispatch(h, vec![Value::Str(to.as_str().to_string())], fx)?;
        }
        Ok(())
    }

    /// The component has mounted: the connection is being made (`connecting`). The status event runs first of everything the
    /// stream does.
    pub(crate) fn stream_start(&mut self, fx: &mut StepEffects) -> Result<(), Fault> {
        self.set_stream_status(StreamStatus::Connecting, fx)
    }

    /// The server accepted the connection: it is a stream, and the next wait is the first again.
    pub fn stream_open(&mut self, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        if let Some(s) = self.stream.as_mut() {
            s.step = 0;
            s.attempt = 0;
        }
        self.set_stream_status(StreamStatus::Open, fx)?;
        Ok(Outcome::Ok)
    }

    /// A connection that was being made or was open ends: the next try is one wait away.
    pub fn stream_drop(&mut self, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        let clock = self.clock;
        if let Some(s) = self.stream.as_mut() {
            let wait = BACKOFF_MS[s.step.min(BACKOFF_MS.len() - 1)];
            s.step = (s.step + 1).min(BACKOFF_MS.len() - 1);
            s.attempt += 1;
            s.retry_due = Some(clock + f64::from(wait));
        }
        self.set_stream_status(StreamStatus::Retrying, fx)?;
        Ok(Outcome::Ok)
    }

    /// The answer was not a stream (a status other than 200, another content type, a redirect): the stream is over.
    pub fn stream_refuse(&mut self, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        self.set_stream_status(StreamStatus::Failed, fx)?;
        Ok(Outcome::Ok)
    }

    /// A message arrives on an open stream. One over `MESSAGE_MAX` characters fails the stream and is not delivered; an `id`
    /// becomes the `Last-Event-ID` of the next connection (an empty one forgets it) before the handler runs.
    pub fn stream_message(
        &mut self,
        text: &str,
        id: Option<&str>,
        fx: &mut StepEffects,
    ) -> Result<Outcome, Fault> {
        if units(text) > MESSAGE_MAX {
            self.set_stream_status(StreamStatus::Failed, fx)?;
            return Ok(Outcome::Ok);
        }
        let Some(s) = self.stream.as_mut() else {
            return Ok(Outcome::Ok);
        };
        if let Some(id) = id {
            s.last_event_id = (!id.is_empty()).then(|| id.to_string());
        }
        let h = s.message;
        self.dispatch(h, vec![Value::Str(text.to_string())], fx)
    }

    /// The time the stream connects again, when it is retrying and not disposed.
    pub(crate) fn stream_retry_due(&self) -> Option<f64> {
        if self.disposed {
            return None;
        }
        self.stream
            .as_ref()
            .filter(|s| s.status == StreamStatus::Retrying)
            .and_then(|s| s.retry_due)
    }

    /// The wait is over: the page connects again.
    pub(crate) fn stream_retry(&mut self, fx: &mut StepEffects) -> Result<(), Fault> {
        if let Some(s) = self.stream.as_mut() {
            s.retry_due = None;
        }
        self.set_stream_status(StreamStatus::Connecting, fx)
    }

    /// The page is hidden: a connection being made, open, or waiting to be made again is closed.
    pub(crate) fn stream_hide(&mut self, fx: &mut StepEffects) -> Result<(), Fault> {
        let live = self.stream.as_ref().is_some_and(|s| {
            matches!(
                s.status,
                StreamStatus::Connecting | StreamStatus::Open | StreamStatus::Retrying
            )
        });
        if !live {
            return Ok(());
        }
        if let Some(s) = self.stream.as_mut() {
            s.retry_due = None;
        }
        self.set_stream_status(StreamStatus::Paused, fx)
    }

    /// The page is visible again: a paused stream connects at once.
    pub(crate) fn stream_show(&mut self, fx: &mut StepEffects) -> Result<(), Fault> {
        if self
            .stream
            .as_ref()
            .is_some_and(|s| s.status == StreamStatus::Paused)
        {
            self.set_stream_status(StreamStatus::Connecting, fx)?;
        }
        Ok(())
    }

    /// The status of the stream, or `None` for a component that does not `subscribe`.
    pub(crate) fn stream_status(&self) -> Option<StreamStatus> {
        self.stream.as_ref().map(|s| s.status)
    }

    /// The `stream` object of a step report (docs/streams.md section 4); `None` for a component that does not subscribe.
    pub fn stream_report(&self) -> Option<serde_json::Value> {
        let s = self.stream.as_ref()?;
        let retry_in = s
            .retry_due
            .filter(|_| s.status == StreamStatus::Retrying)
            .map(|d| crate::value::num_json((d - self.clock).max(0.0)));
        Some(serde_json::json!({
            "status": s.status.as_str(),
            "attempt": s.attempt,
            "retry_in_ms": retry_in,
            "last_event_id": s.last_event_id,
        }))
    }
}
