use super::{Latency, Report, error};
use crate::{Result, benchmark::elapsed};
use ipcengine::{
    message::Connection,
    native::Receiver,
    observe::Monitor,
    stream::{CHUNK, Client},
};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub fn run(root: &str, mode: &str, dimensions: (&str, &str)) -> Result<Report> {
    if !["memory", "file", "shared"].contains(&mode) {
        return Err("unknown channel workload".into());
    }
    let length: usize = dimensions.0.parse().map_err(error)?;
    let iterations: usize = dimensions.1.parse().map_err(error)?;
    if !(1..=16_777_216).contains(&length) || !(1..=64).contains(&iterations) {
        return Err("invalid channel workload".into());
    }
    let root = Path::new(root);
    let socket = UnixStream::connect(root.join("data")).map_err(error)?;
    let control = Connection::connect(&root.join("control")).map_err(error)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let probe = std::thread::spawn(move || probe(control, stopping));
    let result = consume(socket, mode, (length, iterations));
    stop.store(true, Ordering::Relaxed);
    let control = probe.join().map_err(|_| "control probe panicked")??;
    let (monitor, first) = result?;
    let snapshot = monitor.snapshot();
    Ok(Report {
        first: Latency::new(first),
        control: Latency::new(control),
        bytes: snapshot.bytes,
        completed: snapshot.completed,
        waiting: snapshot.waiting,
        active: snapshot.active,
    })
}
fn consume(
    socket: UnixStream,
    mode: &str,
    dimensions: (usize, usize),
) -> Result<(Monitor, Vec<u64>)> {
    let (length, iterations) = dimensions;
    let mut first = Vec::new();
    let monitor;
    if mode == "shared" {
        let mut receiver = Receiver::new(socket).map_err(error)?;
        monitor = receiver.monitor();
        for iteration in 0..iterations {
            let lease = receiver.acquire().map_err(error)?;
            first.push(monitor.snapshot().first.unwrap_or(0));
            if lease.checksum() != length as u64 * (iteration + 1) as u64 {
                return Err("shared payload mismatch".into());
            }
            lease.finish().map_err(error)?;
        }
    } else {
        monitor = Monitor::default();
        let mut connection = Connection::new(socket).map_err(error)?;
        for iteration in 0..iterations {
            let mut client = Client::new(&mut connection, length as u64, &monitor);
            let sum = drain(&mut client)?;
            first.push(monitor.snapshot().first.unwrap_or(0));
            if sum != length as u64 * (iteration + 1) as u64 {
                return Err("stream payload mismatch".into());
            }
            client.close().map_err(error)?;
        }
    }
    Ok((monitor, first))
}
fn probe(mut connection: Connection, stop: Arc<AtomicBool>) -> Result<Vec<u64>> {
    let mut samples = Vec::new();
    let mut nonce = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        connection.write(&nonce, 64).map_err(error)?;
        let reply: u64 = connection.read(64, Duration::from_secs(6)).map_err(error)?;
        if reply != nonce {
            return Err("control correlation mismatch".into());
        }
        samples.push(elapsed(started));
        nonce += 1;
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(samples)
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().map(|byte| u64::from(*byte)).sum()
}

fn drain(client: &mut Client<'_>) -> Result<u64> {
    let mut sum = 0;
    loop {
        let chunk = client.read(CHUNK).map_err(error)?;
        sum += checksum(&chunk.bytes);
        if chunk.eof {
            return Ok(sum);
        }
    }
}
