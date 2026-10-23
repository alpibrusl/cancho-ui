//! The URL fragment and the history the page would have (docs/routing.md section 5): what `router.go`, `router.replace` and
//! the trace steps `navigate`, `back` and `forward` act on. The route event runs for each path in `queued`, after the
//! handler that queued it returns, as the browser fires `hashchange` after the task that set the fragment.

use std::collections::VecDeque;

/// The most route events one step may run: a handler that navigates to a new path every time it runs cannot make replay
/// run (and report) forever.
pub const MAX_ROUTE_EVENTS_PER_STEP: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct Router {
    /// The entries of the history, oldest first; `position` is the current one.
    pub entries: Vec<String>,
    pub position: usize,
    /// The paths whose route event has not run yet.
    pub queued: VecDeque<String>,
}

impl Router {
    pub fn new(initial: &str) -> Router {
        Router {
            entries: vec![initial.to_string()],
            position: 0,
            queued: VecDeque::new(),
        }
    }

    pub fn current(&self) -> &str {
        self.entries.get(self.position).map_or("/", String::as_str)
    }

    /// `router.go(path)`: a new entry after the current one (the entries after it are dropped, as a browser does). The same
    /// path as the current one changes nothing and fires nothing. The caller has checked the path is legal.
    pub fn push(&mut self, path: &str) {
        if self.current() == path {
            return;
        }
        self.entries.truncate(self.position + 1);
        self.entries.push(path.to_string());
        self.position += 1;
        self.queued.push_back(path.to_string());
    }

    /// `router.replace(path)`: the current entry becomes `path`.
    pub fn replace(&mut self, path: &str) {
        if self.current() == path {
            return;
        }
        if let Some(entry) = self.entries.get_mut(self.position) {
            *entry = path.to_string();
        }
        self.queued.push_back(path.to_string());
    }

    pub fn can_back(&self) -> bool {
        self.position > 0
    }

    pub fn can_forward(&self) -> bool {
        self.position + 1 < self.entries.len()
    }

    /// The browser's Back: the previous entry becomes current. Its path is delivered when it differs from the one left.
    pub fn back(&mut self) {
        if self.can_back() {
            let left = self.current().to_string();
            self.position -= 1;
            if self.current() != left {
                self.queued.push_back(self.current().to_string());
            }
        }
    }

    pub fn forward(&mut self) {
        if self.can_forward() {
            let left = self.current().to_string();
            self.position += 1;
            if self.current() != left {
                self.queued.push_back(self.current().to_string());
            }
        }
    }
}
