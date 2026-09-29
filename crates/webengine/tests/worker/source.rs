use webengine::{
    Fault,
    resource::{Provider, Response},
};

pub(super) struct Source;

impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if request.url.path().ends_with("main.js") {
            return Ok(Response {
                status: 200,
                mime: "text/javascript".into(),
                body: fixtures::script("document, bridge", "").into_bytes(),
            });
        }
        Ok(Response { status:200,mime:"text/html".into(),body:b"<head><script src='./main.js'></script></head><body id='body'><button id='a' style='height: 32px; background-color: #112233;'>A</button><button id='b' style='height: 32px; background-color: #445566;'>B</button><button id='c' style='height: 32px;'>C</button></body>".to_vec() })
    }
}
