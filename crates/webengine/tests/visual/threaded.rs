use graphics::shared::{Extent, Output, Preparation};
use std::time::{Duration, Instant};
use webengine::{Action, Fault, Phase, State, Worker, resource::Provider};

#[derive(Clone)]
struct Application(fixtures::Application);

impl Provider for Application {
    fn request(
        &mut self,
        request: &webengine::resource::Request,
    ) -> Result<webengine::resource::Response, Fault> {
        let resource = self.0.load(request.url.path()).map_err(Fault)?;
        request.accept(webengine::resource::Response {
            status: 200,
            mime: resource.mime,
            body: resource.body,
        })
    }
}

fn settle(worker: &mut Worker, preparation: &mut Preparation, job: Option<u64>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.poll().unwrap();
        if let Some(frame) = worker.frame() {
            preparation
                .prepare(&frame, Output::new(frame.extent(), 1.0).unwrap())
                .unwrap();
            worker.acknowledge(frame.id()).unwrap();
        }
        let snapshot = worker.inspect();
        let jobs = &snapshot.execution.as_ref().unwrap().jobs;
        let complete = job.map_or_else(
            || {
                jobs.iter()
                    .all(|entry| matches!(entry.phase(), Phase::Complete | Phase::Cancelled))
            },
            |job| {
                jobs.iter()
                    .any(|entry| entry.id == job && entry.phase() == Phase::Complete)
            },
        );
        if snapshot.state == State::Ready && complete {
            return;
        }
        assert!(Instant::now() < deadline, "visual worker did not settle");
        std::thread::yield_now();
    }
}

fn evaluate(worker: &mut Worker, preparation: &mut Preparation, source: &str) -> String {
    let job = worker.submit(Action::Evaluate(source.into())).unwrap();
    settle(worker, preparation, Some(job));
    worker
        .inspect()
        .execution
        .unwrap()
        .jobs
        .iter()
        .find(|entry| entry.id == job)
        .unwrap()
        .result()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone()
}

#[test]
fn scrolling() {
    let config = config::Store::default();
    config.set("layout.convergence", "deferred".into());
    let mut worker = Worker::configured(
        "app://visual/index.html",
        Extent {
            width: 720,
            height: 640,
        },
        Application(fixtures::Application::new("visual")),
        config,
    )
    .unwrap();
    let mut preparation = Preparation::default();
    settle(&mut worker, &mut preparation, None);
    evaluate(&mut worker, &mut preparation, "scenario.start();");
    for _ in 0..24 {
        worker.wheel(360, 360, 160).unwrap();
        settle(&mut worker, &mut preparation, None);
    }
    let range = evaluate(&mut worker, &mut preparation, "scenario.range();");
    let values = range
        .split(':')
        .map(|value| value.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert!(values[0] > 0, "range={range}");
    assert!(values[1] - values[0] < 64);
    worker.close();
}
