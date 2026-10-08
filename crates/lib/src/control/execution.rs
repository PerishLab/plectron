use super::protocol::{Action, Report, Task};
use super::proxy::Proxy;
use super::publication::Publisher;
use crate::addons::control::{Control, Dispatch};
use crate::contract::Fault;
use crate::platform::Host;
use crate::runtime::Runtime;
use std::sync::mpsc::Receiver;

pub(super) fn run(proxy: Proxy, inbox: Receiver<Task>, outbox: Publisher) {
    let Ok(mut runtime) = Runtime::new(Control::new(proxy)) else {
        return;
    };
    for task in inbox {
        let mut scope = task.context.enter("control.execute");
        if outbox.send(Report::Started(task.id)).is_err() {
            break;
        }
        let id = task.id;
        let result = execute(&mut runtime, task, &outbox);
        let success = result.is_ok();
        let published = observe::call("control.publish", || {
            outbox
                .send(Report::Finished(
                    id,
                    result.map_err(|error| error.to_string()),
                    runtime.activity(),
                    runtime.performance(),
                    std::time::Instant::now(),
                ))
                .map_err(|_| ())
        });
        scope.finish(success && published.is_ok());
        if published.is_err() {
            break;
        }
    }
}
fn execute(
    runtime: &mut Runtime<Control<Proxy>>,
    task: Task,
    outbox: &Publisher,
) -> Result<String, Fault> {
    match task.action {
        Action::Hold(release) => {
            outbox
                .send(Report::Held(task.id))
                .map_err(|_| Fault("control disconnected".into()))?;
            release
                .wait()
                .map(|()| String::new())
                .map_err(|error| Fault(error.to_string()))
        }
        Action::Startup(source) => evaluate(runtime, &source),
        Action::Evaluate(source) => evaluate(runtime, &source.as_str().into()),
        Action::Dispatch(signal) => runtime.dispatch(signal).map(|()| String::new()),
    }
}
fn evaluate(
    runtime: &mut Runtime<Control<Proxy>>,
    source: &crate::runtime::source::Input,
) -> Result<String, Fault> {
    let display = source
        .execute(runtime)
        .map_err(|failure| runtime.diagnostic(&failure))?
        .text();
    runtime
        .checkpoint()
        .map_err(|failure| runtime.diagnostic(&failure))?;
    runtime.addons().host.finish()?;
    Ok(display)
}
