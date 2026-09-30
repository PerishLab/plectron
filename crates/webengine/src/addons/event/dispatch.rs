use super::{
    Data, Web,
    object::{self, Context},
    record::{Delivery, Outcome, Phase},
};
use crate::{Fault, error};
use domengine::Node;
use jsengine::{Engine, Failure, Value};

pub fn emit(
    engine: &mut Engine,
    web: &mut Web,
    target: Node,
    data: Data,
) -> Result<Outcome, Fault> {
    let path = web.document.path(target).map_err(error)?;
    web.events.begin(&path, &data)?;
    let result = object::event(engine, web, target, &data)
        .and_then(|event| walk(engine, web, &path, (&data, event)));
    let outcome = web
        .events
        .finish(result.as_ref().err().map(ToString::to_string));
    result.map(|()| outcome)
}
fn walk(
    engine: &mut Engine,
    web: &mut Web,
    path: &[Node],
    event: (&Data, Value),
) -> Result<(), Fault> {
    let (data, value) = event;
    for node in path.iter().skip(1).rev() {
        visit(engine, web, (*node, Phase::Capture, true), (data, &value))?;
        if web.events.frame().stopped {
            return Ok(());
        }
    }
    visit(engine, web, (path[0], Phase::Target, true), (data, &value))?;
    if !web.events.frame().immediate {
        visit(engine, web, (path[0], Phase::Target, false), (data, &value))?;
    }
    if !data.bubbles || web.events.frame().stopped {
        return Ok(());
    }
    for node in path.iter().skip(1) {
        visit(engine, web, (*node, Phase::Bubble, false), (data, &value))?;
        if web.events.frame().stopped {
            break;
        }
    }
    Ok(())
}
fn visit(
    engine: &mut Engine,
    web: &mut Web,
    point: (Node, Phase, bool),
    event: (&Data, &Value),
) -> Result<(), Fault> {
    let (node, phase, capture) = point;
    let (data, event) = event;
    let ids = web.subscriptions.snapshot(node, &data.kind, capture);
    for id in ids {
        let Some(subscription) = web.subscriptions.take(id) else {
            continue;
        };
        let lease = web.events.enter(id)?;
        let result = object::context(
            engine,
            web,
            Context {
                node,
                phase,
                lease,
                observe: subscription.options.observe,
            },
        )
        .map_err(Failure::Fault)
        .and_then(|context| engine.call(subscription.callback, vec![event.clone(), context], web));
        let diagnostic = result
            .as_ref()
            .err()
            .map(|failure| engine.diagnostic(failure).to_string());
        let frame = web.events.frame();
        frame.current = None;
        if result
            .as_ref()
            .is_err_and(|failure| failure.thrown().is_some())
        {
            frame.record.errors += 1;
            if frame.record.exception.is_none() {
                frame.record.exception = diagnostic
                    .as_ref()
                    .map(|text| text.chars().take(256).collect());
            }
        }
        if frame.record.deliveries.len() < 64 {
            frame.record.deliveries.push(Delivery {
                subscription: id,
                node: node.id(),
                phase,
                error: diagnostic.map(|text| text.chars().take(256).collect()),
            });
        } else {
            frame.record.omitted += 1;
        }
        if let Err(failure) = result
            && failure.thrown().is_none()
        {
            return Err(engine.diagnostic(&failure));
        }
        if frame.immediate {
            break;
        }
    }
    Ok(())
}
