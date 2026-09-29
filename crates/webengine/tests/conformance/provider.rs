use super::*;
impl Provider for Source {
    fn request(&mut self, request: &webengine::resource::Request) -> Result<Response, Fault> {
        if matches!(request.kind, webengine::resource::Kind::Font) {
            let name = request.url.path().rsplit('/').next().unwrap();
            return Ok(Response {
                status: 200,
                mime: "font/ttf".into(),
                body: ::fixtures::bytes(name).to_vec(),
            });
        }
        let response: Result<Response, Fault> = {
            let url = &request.url;
            self.requested.push(url.to_string());
            let file = self
                .files
                .get(url.path())
                .ok_or_else(|| Fault("missing resource".into()))?;
            let body = if file.mime == "text/javascript" {
                fixtures::script(
                    "document, bridge, events, shortcuts, fetch, requests",
                    &file.body,
                )
                .into_bytes()
            } else {
                file.body.as_bytes().to_vec()
            };
            Ok(Response {
                status: 200,
                mime: file.mime.clone(),
                body,
            })
        };
        let mut response = response?;
        if response.mime == "text/css" {
            response.body = ::fixtures::stylesheet(&response.body);
        }
        Ok(response)
    }
}
