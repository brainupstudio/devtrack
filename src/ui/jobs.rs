//! One background job at a time for connector calls; results come back through a channel.

use devtrack_core::integrations::{self, RemoteDetail, RemoteItem, RemoteProject, SResult, WorkSource};
use devtrack_core::model::{Id, IntegrationConfig};
use devtrack_core::secrets;
use std::sync::mpsc::{channel, Receiver};

pub enum JobOut {
    Test { cfg: Id, res: SResult<String> },
    /// A query: all matching ids, the first page of details, and (when asked) the source's
    /// projects so the filter is ready without a separate step.
    Found { cfg: Id, res: SResult<(Vec<String>, Vec<RemoteItem>, Option<Vec<RemoteProject>>)> },
    /// The next page of details.
    More { cfg: Id, res: SResult<Vec<RemoteItem>> },
    /// Refresh of imported tasks: which ids were asked for and what came back.
    Refresh { cfg: Id, asked: Vec<String>, res: SResult<Vec<RemoteItem>> },
    /// Read-only work detail of one imported task (remaining, completed, last updates).
    Detail { task: Id, res: SResult<RemoteDetail> },
}

#[derive(Default)]
pub struct Jobs {
    rx: Option<Receiver<JobOut>>,
    pub label: Option<String>,
}

impl Jobs {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Start a job. The closure gets the connector, or the reason it couldn't be built.
    pub fn spawn<F>(&mut self, cfg: &IntegrationConfig, label: &str, f: F) -> bool
    where
        F: FnOnce(SResult<Box<dyn WorkSource>>) -> JobOut + Send + 'static,
    {
        if self.busy() {
            return false;
        }
        let secret = secrets::get(&cfg.id);
        let cfg = cfg.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let conn = integrations::connect(&cfg, secret);
            let _ = tx.send(f(conn));
        });
        self.rx = Some(rx);
        self.label = Some(label.into());
        true
    }

    pub fn poll(&mut self) -> Option<JobOut> {
        let out = self.rx.as_ref()?.try_recv().ok()?;
        self.rx = None;
        self.label = None;
        Some(out)
    }
}
