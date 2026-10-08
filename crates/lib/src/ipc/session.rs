mod command;
mod input;
mod publication;
use crate::control::Adapter;
use std::collections::VecDeque;

use super::{Command, Mode, Reply, Request, Response, Window};
use crate::addons::control::Control;
use crate::contract::{Event, Fault, Signal};
use crate::platform::{Backend, Evaluation, Host, Platform};
use crate::rendering::Renderer;
use crate::runtime::Runtime;

pub struct Session<B, R> {
    records: Vec<publication::Pending>,
    observation: super::observation::Recorder,
    runtime: Adapter<B, R>,
    pid: u32,
    mode: Mode,
    persistent: bool,
    queue: VecDeque<Signal>,
    dropped: u64,
    fault: Option<String>,
}

impl<B: Backend, R: Renderer> Session<B, R> {
    pub fn new(runtime: Runtime<Control<Platform<B, R>>>, pid: u32, persistent: bool) -> Self {
        Self {
            records: Vec::new(),
            observation: Default::default(),
            runtime: Adapter::Local(Box::new(runtime)),
            pid,
            persistent,
            mode: Mode::Running,
            queue: VecDeque::new(),
            dropped: 0,
            fault: None,
        }
    }

    pub fn spawn(
        platform: Platform<B, R>,
        source: impl Into<crate::runtime::source::Input>,
        process: (u32, bool),
        notify: Option<shared::Notify>,
    ) -> Result<Self, Fault> {
        let mut runtime = Adapter::spawn(platform, source, notify)?;
        runtime.host().prepare()?;
        Ok(Self {
            records: Vec::new(),
            observation: Default::default(),
            runtime,
            pid: process.0,
            persistent: process.1,
            mode: Mode::Running,
            queue: VecDeque::new(),
            dropped: 0,
            fault: None,
        })
    }

    pub fn active(&mut self) -> bool {
        if self.mode == Mode::Stopping {
            return false;
        }
        if self.persistent || self.mode == Mode::Paused {
            return true;
        }
        !self.queue.is_empty() || self.runtime.busy() || self.runtime.host().active()
    }

    pub fn handle(&mut self, request: Request) -> Response {
        let mut scope = (!matches!(request.command, Command::Observe { .. }))
            .then(|| self.observation.scope(request.command.name()))
            .flatten();
        let admitted = std::time::Instant::now();
        let target = publication::target(&request.command);
        let mut result = if request.version != 1 {
            Err(Fault(format!(
                "unsupported protocol version: {}",
                request.version
            )))
        } else if self.mode == Mode::Stopping {
            Err(Fault("session is stopping".into()))
        } else {
            self.command(request.command)
        };
        if let Ok(Reply::Accepted { job, record }) = &mut result {
            match self.track(target, *job, admitted) {
                Ok(id) => *record = Some(id),
                Err(error) => self.fault = Some(error.to_string()),
            }
        }
        self.publish();
        if let Some(scope) = &mut scope {
            scope.finish(result.is_ok());
        }
        Response {
            version: 1,
            result: result.map_err(|error| error.to_string()),
        }
    }

    pub fn tick(&mut self) -> Result<(), Fault> {
        let mut scope = self.observation.scope("application.tick");
        let result = self.advance();
        self.publish();
        if let Some(scope) = &mut scope {
            scope.finish(result.is_ok());
        }
        result
    }

    pub fn wait(&mut self) -> Result<(), Fault> {
        if self.queue.is_empty() {
            self.runtime.host().wait()?;
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), Fault> {
        if self.mode == Mode::Stopping {
            return Ok(());
        }
        if let Some(error) = self.runtime.poll()? {
            self.fault = Some(error);
            self.mode = Mode::Paused;
        }
        for signal in self.runtime.host().poll()? {
            if signal.event == Event::Close {
                self.queue.retain(|item| item.unit != signal.unit);
            }
            if signal.event.input() && self.queue.len() >= 256 {
                self.dropped = self.dropped.saturating_add(1);
            } else {
                self.queue.push_back(signal);
            }
        }
        if self.mode == Mode::Running
            && let Err(error) = self.step()
        {
            self.fault = Some(error.to_string());
            self.mode = Mode::Paused;
        }
        Ok(())
    }

    fn windows(&mut self) -> Vec<Window> {
        self.runtime
            .host()
            .windows()
            .into_iter()
            .map(|(unit, window)| Window {
                id: unit.id(),
                title: window.title,
                width: window.extent.width,
                height: window.extent.height,
            })
            .collect()
    }

    fn step(&mut self) -> Result<bool, Fault> {
        let target = self
            .queue
            .iter()
            .position(|signal| signal.event.input() && self.runtime.host().attached(signal.unit));
        if let Some(index) = target {
            let signal = self.queue.remove(index).expect("queued target");
            self.runtime.host().input(&signal)?;
            self.runtime.host().finish()?;
            return Ok(true);
        }
        if !self.runtime.available() {
            return self.runtime.host().delivery();
        }
        let Some(signal) = self.queue.pop_front() else {
            return self.runtime.host().delivery();
        };
        self.runtime.dispatch(signal)?;
        Ok(true)
    }

    fn shutdown(&mut self) -> Reply {
        self.mode = Mode::Stopping;
        self.runtime.close();
        self.queue.clear();
        let (_, failures) = self.runtime.host().shutdown();
        let errors = failures
            .into_iter()
            .map(|error| error.to_string())
            .collect();
        self.runtime.host().release();
        self.runtime.host().query().cancel(None);
        Reply::Shutdown { errors }
    }
}
