use super::Case;
use webengine::resource::{Provider, Request, Response};
use webengine::{Fault, Page};

struct Synthetic;

impl Provider for Synthetic {
    fn request(&mut self, request: &Request) -> Result<Response, Fault> {
        let (mime, body) = match request.url.path() {
            "/index.html" => (
                "text/html",
                "<html><head><link rel='stylesheet' href='./style.css'></head><body><p-input id='editor'></p-input><button>Add task</button></body></html>",
            ),
            "/style.css" => (
                "text/css",
                "@keyframes blink {from {opacity: 1;} 50% {opacity: 0;} to {opacity: 1;}} body {background-color: #e8edf5;} p-input {width: 960px; height: 64px; overflow: hidden; background-color: #c4cff8;} button {width: 240px; height: 64px; background-color: #5145e5;} p-caret {animation-name: blink; animation-duration: 1200ms; animation-iteration-count: infinite; animation-timing-function: step-end;}",
            ),
            _ => return Err(Fault("unexpected caret benchmark resource".into())),
        };
        Ok(Response {
            status: 200,
            mime: mime.into(),
            body: body.as_bytes().to_vec(),
        })
    }
}

impl Case {
    pub(super) fn page(self) -> Result<Page, Fault> {
        match self.id {
            "isolated" => self.isolated(),
            "todo" => self.todo(),
            _ => Err(Fault("unknown caret workload".into())),
        }
    }

    fn isolated(self) -> Result<Page, Fault> {
        let mut page = Page::new("app://caret/index.html", self.extent())?;
        page.load(&mut Synthetic)?;
        page.evaluate(&fixtures::script(
            "document",
            "const editor = document.getElementById('editor'); const text = document.createTextNode('A small list for real work.'); const caret = document.createElement('p-caret'); editor.appendChild(text); editor.appendChild(caret); document.attachEditor(editor, text, caret); editor.focus();",
        ))?;
        ready(page)
    }

    fn todo(self) -> Result<Page, Fault> {
        let mut page = Page::new("app://scenarios/todo/src/index.html", self.extent())?;
        page.load(&mut crate::scenario::Source::default())?;
        ready(page)
    }

    fn extent(self) -> domengine::Extent {
        domengine::Extent {
            width: self.width,
            height: self.height,
        }
    }
}

fn ready(mut page: Page) -> Result<Page, Fault> {
    page.render()?;
    let frame = page
        .frame()
        .ok_or_else(|| Fault("caret benchmark did not produce initial frame".into()))?;
    page.acknowledge(frame.id())?;
    Ok(page)
}
