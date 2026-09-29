use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
mod observation;
use std::time::{Duration, Instant};
use webengine::{
    Action, Fault, Page, Worker,
    resource::{Cancellation, Provider, Request, Response, Transport},
};

#[derive(Default)]
struct Remote {
    released: AtomicBool,
    entered: AtomicUsize,
    cancelled: AtomicUsize,
}
impl Transport for Remote {
    fn send(&self, request: &Request, cancellation: &Cancellation) -> Result<Response, Fault> {
        if request.url.path() == "/slow" {
            self.entered.fetch_add(1, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(3);
            while !self.released.load(Ordering::SeqCst) && Instant::now() < deadline {
                if cancellation.cancelled() {
                    self.cancelled.fetch_add(1, Ordering::SeqCst);
                    return cancellation.check().map(|()| unreachable!());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if request.url.path() == "/error" {
            return Err(Fault("connection lost".into()));
        }
        if request.url.path() == "/json" {
            return Ok(Response {
                status: 200,
                mime: "application/json".into(),
                body: r#"{"name":"中文","items":[null,{"enabled":true}],"count":3.5}"#
                    .as_bytes()
                    .to_vec(),
            });
        }
        if request.url.path() == "/malformed-json" {
            return Ok(Response {
                status: 200,
                mime: "application/json".into(),
                body: br#"{"broken":]"#.to_vec(),
            });
        }
        Ok(Response {
            status: if request.url.path() == "/missing" {
                404
            } else {
                200
            },
            mime: "text/plain".into(),
            body: b"hello".to_vec(),
        })
    }
}
struct Source(Arc<Remote>);
impl Provider for Source {
    fn transport(&self) -> Option<Arc<dyn Transport>> {
        Some(self.0.clone())
    }
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document, requests, fetch", "").into_bytes(),
            });
        }
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<head><script src='./main.js'></script></head><button id='button'>Ready</button><p id='status'>Idle</p>".to_vec(),
        })
    }
}
fn extent() -> graphics::shared::Extent {
    graphics::shared::Extent {
        width: 200,
        height: 120,
    }
}
fn until(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !done() {
        assert!(Instant::now() < deadline, "request did not progress");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn setup() -> &'static str {
    "const button = document.getElementById('button'); const status = document.getElementById('status'); let clicks = 0; button.addEventListener('click', () => { clicks++; button.setText('Clicks: ' + clicks); }); let scope = requests.scope(); const load = async () => { status.setText('Loading'); try { const response = await scope.fetch('/slow'); status.setText(await response.text()); } catch (error) { status.setText('Cancelled'); } };"
}
#[test]
fn lifecycle() {
    use graphics::shared::{Backend, Cpu};
    let remote = Arc::new(Remote::default());
    let mut page = Page::new("app://test/index.html", extent()).unwrap();
    page.load(&mut Source(remote.clone())).unwrap();
    page.evaluate(setup()).unwrap();
    page.evaluate("load();").unwrap();
    page.render().unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Loading")
    );
    let old = page.frame().unwrap();
    let pixels = Cpu.render(&old).unwrap();
    page.acknowledge(old.id()).unwrap();
    until(|| remote.entered.load(Ordering::SeqCst) == 1);
    page.click(5, 5).unwrap();
    page.render().unwrap();
    assert_eq!(page.evaluate("clicks;").unwrap(), "1");
    page.evaluate("scope.cancel();").unwrap();
    page.render().unwrap();
    assert_eq!(page.inspect().requests.cancelled, 1);
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Cancelled")
    );
    until(|| remote.cancelled.load(Ordering::SeqCst) == 1);
    page.evaluate("scope = requests.scope(); load();").unwrap();
    page.render().unwrap();
    remote.released.store(true, Ordering::SeqCst);
    until(|| {
        page.poll().unwrap();
        page.inspect().requests.pending == 0
    });
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "hello")
    );
    assert_eq!(Cpu.render(&old).unwrap(), pixels);
    page.evaluate("let result = ''; const probe = async () => { const response = await fetch('/missing'); result += response.status; result += await response.text(); try { await response.text(); } catch { result += 'used'; } try { await fetch('/error'); } catch { result += 'failed'; } }; probe();").unwrap();
    page.render().unwrap();
    until(|| {
        page.poll().unwrap();
        page.evaluate("result;").unwrap() == "404hellousedfailed"
    });
    page.evaluate("let jsonResult = ''; const decode = async () => { const response = await fetch('/json'); const value = await response.json(); jsonResult += value.name + value.items[0] + value.items[1].enabled + value.count; try { await response.text(); } catch { jsonResult += 'used'; } try { const malformed = await fetch('/malformed-json'); await malformed.json(); } catch { jsonResult += 'invalid'; } }; decode();").unwrap();
    page.render().unwrap();
    until(|| {
        page.poll().unwrap();
        page.evaluate("jsonResult;").unwrap() == "中文nulltrue3.5usedinvalid"
    });
    assert!(page.inspect().jobs.rejected.is_empty());
    observation::complete(&mut page);
    remote.released.store(false, Ordering::SeqCst);
    page.evaluate("load();").unwrap();
    page.render().unwrap();
    until(|| remote.entered.load(Ordering::SeqCst) == 3);
    page.close();
    until(|| remote.cancelled.load(Ordering::SeqCst) == 2);
    until(|| Arc::strong_count(&remote) == 1);
    assert_eq!(Cpu.render(&old).unwrap(), pixels);
}
#[test]
fn worker() {
    let remote = Arc::new(Remote::default());
    let mut worker =
        Worker::spawn("app://test/index.html", extent(), Source(remote.clone())).unwrap();
    until(|| {
        worker.poll().unwrap();
        worker.state() == webengine::State::Ready
    });
    let start = worker
        .submit(Action::Evaluate(format!("{} load();", setup())))
        .unwrap();
    until(|| {
        worker.poll().unwrap();
        worker
            .inspect()
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == start && job.phase() == webengine::Phase::Complete)
    });
    until(|| remote.entered.load(Ordering::SeqCst) == 1);
    let job = worker
        .submit(Action::Evaluate("button.setText('Responsive');".into()))
        .unwrap();
    until(|| {
        worker.poll().unwrap();
        worker
            .inspect()
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|entry| entry.id == job && entry.phase() == webengine::Phase::Complete)
    });
    assert!(
        worker
            .inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Responsive")
    );
    let hold = worker.hold().unwrap();
    until(|| {
        worker.poll().unwrap();
        worker
            .inspect()
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == webengine::Phase::Held)
    });
    remote.released.store(true, Ordering::SeqCst);
    assert!(
        worker
            .inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Loading")
    );
    worker.unhold().unwrap();
    until(|| {
        worker.poll().unwrap();
        worker
            .inspect()
            .nodes
            .iter()
            .any(|node| node.content == "hello")
    });
    assert!(worker.errors().is_empty());
    assert!(worker.inspect().jobs.rejected.is_empty());
    worker.close();
}
