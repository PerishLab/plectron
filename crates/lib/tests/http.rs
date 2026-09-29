use plectron::resource::Services;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use webengine::resource::{Cancellation, Provider, Request, address};

fn head(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        data.push(byte[0]);
    }
    String::from_utf8(data).unwrap()
}
#[test]
fn framing() {
    for (response, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Type: text/plain\r\n\r\nhello",
            Some((200, "hello")),
        ),
        (
            "HTTP/1.1 404 Missing\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhe\r\n3\r\nllo\r\n0\r\n\r\n",
            Some((404, "hello")),
        ),
        ("HTTP/1.1 200 OK\r\n\r\nhello", Some((200, "hello"))),
        ("HTTP/1.1 204 Empty\r\n\r\n", Some((204, ""))),
        ("HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nshort", None),
        ("HTTP/1.1 200 OK\r\nContent-Length: 65537\r\n\r\n", None),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc!!",
            None,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let header = head(&mut stream);
            assert!(header.starts_with("GET /data?x=1 HTTP/1.1\r\n"));
            assert!(header.contains(&format!("Host: {endpoint}\r\n")));
            stream.write_all(response.as_bytes()).unwrap();
        });
        let response =
            Services::default().get(&address(&format!("http://{endpoint}/data?x=1")).unwrap());
        match expected {
            Some((status, body)) => {
                let response = response.unwrap();
                assert_eq!(response.status, status);
                assert_eq!(response.body, body.as_bytes());
            }
            None => assert!(response.is_err()),
        }
        worker.join().unwrap();
    }
}
#[test]
fn cancellation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        head(&mut stream);
        sender.send(()).unwrap();
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let cancellation = Cancellation::default();
    let signal = cancellation.clone();
    let client = std::thread::spawn(move || {
        let transport = Services::default().transport().unwrap();
        transport.send(
            &Request {
                kind: webengine::resource::Kind::Text,
                url: address(&format!("http://{endpoint}/slow")).unwrap(),
            },
            &signal,
        )
    });
    receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    cancellation.cancel();
    assert!(
        client
            .join()
            .unwrap()
            .err()
            .unwrap()
            .0
            .contains("cancelled")
    );
    server.join().unwrap();
}

#[test]
fn body() {
    let source = vec![b'x'; 60_000];
    let expected = source.clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        head(&mut stream);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            source.len()
        )
        .unwrap();
        for chunk in source.chunks(4096) {
            stream.write_all(chunk).unwrap();
        }
    });
    let response = Services::default()
        .get(&address(&format!("http://{endpoint}/body")).unwrap())
        .unwrap();
    assert_eq!(response.body, expected);
    server.join().unwrap();
}
