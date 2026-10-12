use super::*;

impl<B: Backend, R: Renderer> Session<B, R> {
    pub(super) fn command(&mut self, command: Command) -> Result<Reply, Fault> {
        match command {
            Command::Query { .. } => Err(Fault("passive queries require the query service".into())),
            Command::Observe { command } => self.observation.command(command),
            Command::Config { command } => Ok(Reply::Config {
                entries: self.runtime.host().config().command(command),
            }),
            Command::Cache { unit, id, command } => self.cache(unit, id, command),
            Command::Status {} => Ok(Reply::Status {
                pid: self.pid,
                mode: self.mode,
                windows: self.runtime.host().windows().len(),
                queued: self.queue.len() + self.runtime.host().waiting(),
                dropped: self.dropped,
                fault: self.fault.clone(),
            }),
            Command::Windows {} => Ok(Reply::Windows {
                entries: self.windows(),
            }),
            Command::Services {} => Ok(Reply::Services {
                entries: self.runtime.host().services(),
            }),
            Command::Page { unit } | Command::Export { unit } => {
                let unit = self.runtime.host().unit(unit)?;
                Ok(Reply::Page {
                    snapshot: Box::new(self.runtime.host().content(unit)?),
                })
            }
            Command::Evaluate { unit, source } => {
                let unit = self.runtime.host().unit(unit)?;
                let display = self.runtime.host().evaluate(unit, &source)?;
                self.runtime.host().finish()?;
                match display {
                    Evaluation::Value(display) => Ok(Reply::Evaluation { display }),
                    Evaluation::Queued(job) => Ok(Reply::Accepted { job, record: None }),
                }
            }
            Command::Collect { unit } => {
                let unit = self.runtime.host().unit(unit)?;
                let display = self.runtime.host().collect(unit)?;
                self.runtime.host().finish()?;
                match display {
                    Evaluation::Value(display) => Ok(Reply::Evaluation { display }),
                    Evaluation::Queued(job) => Ok(Reply::Accepted { job, record: None }),
                }
            }
            Command::Pointer { unit, x, y } => self.point(unit, x, y),
            Command::Edit { unit, edit } => self.edit(unit, edit),
            Command::Input { unit, input } => self.input(unit, input),
            Command::Composition { unit, input } => self.composition(unit, input),
            Command::Scroll { unit, delta } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().scroll(unit, delta)?;
                Ok(Reply::Ack)
            }
            Command::Wheel { unit, x, y, delta } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().wheel(unit, (x, y), delta)?;
                Ok(Reply::Ack)
            }
            Command::Pan {
                unit,
                scene,
                space,
                delta,
            } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().pan(unit, (scene, space), delta)?;
                Ok(Reply::Ack)
            }
            Command::Send { unit, text } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().message(unit, &text)?;
                Ok(Reply::Ack)
            }
            Command::Hold { unit: Some(unit) } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime
                    .host()
                    .hold(unit)
                    .map(|job| Reply::Accepted { job, record: None })
            }
            Command::Unhold { unit: Some(unit) } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().unhold(unit)?;
                Ok(Reply::Ack)
            }
            Command::Pause {} => {
                self.mode = Mode::Paused;
                self.manual = true;
                Ok(Reply::Ack)
            }
            Command::Resume {} => {
                self.mode = Mode::Running;
                self.manual = false;
                self.fault = None;
                Ok(Reply::Ack)
            }
            Command::Step {} => {
                if self.mode != Mode::Paused {
                    return Err(Fault("step requires a paused session".into()));
                }
                self.step().map(|dispatched| Reply::Stepped { dispatched })
            }
            Command::Eval { source } => match self.runtime.evaluate(&source)? {
                Evaluation::Value(display) => Ok(Reply::Evaluation { display }),
                Evaluation::Queued(job) => Ok(Reply::Accepted { job, record: None }),
            },
            Command::Control {} => self
                .runtime
                .inspect()
                .map(|snapshot| Reply::Control { snapshot }),
            Command::Hold { unit: None } => self
                .runtime
                .hold()
                .map(|job| Reply::Accepted { job, record: None }),
            Command::Unhold { unit: None } => {
                self.runtime.unhold()?;
                Ok(Reply::Ack)
            }
            Command::Click { unit } => self.click(unit),
            Command::Close { unit } => {
                let unit = self.runtime.host().unit(unit)?;
                self.runtime.host().close(unit)?;
                self.queue.retain(|signal| signal.unit != unit);
                Ok(Reply::Ack)
            }
            Command::Shutdown {} => Ok(self.shutdown()),
        }
    }
}
