use super::{Context, Sample, error, process::Process};
use crate::{Result, benchmark::elapsed};
use ipcengine::{
    file::Draft, message::Connection, native::Publisher, observe::Monitor, stream::Transfer,
};
use std::io::{Cursor, Write};
use std::time::Instant;

pub fn run(context: &Context, mode: &str, length: usize, repetition: usize) -> Result<Sample> {
    let started = Instant::now();
    let iterations = 8;
    let setup;
    let mut process = Process::new(context, mode, length, iterations)?;
    let socket = process.data.take().expect("data socket");
    let mut preparation = 0;
    let mut delivery = 0;
    let monitor;
    if mode == "shared" {
        let mut publisher = Publisher::new(socket, &process.root, length).map_err(error)?;
        monitor = publisher.monitor();
        setup = elapsed(started);
        for iteration in 0..iterations {
            let started = Instant::now();
            publisher.fill((iteration + 1) as u8).map_err(error)?;
            preparation += elapsed(started);
            let started = Instant::now();
            publisher.publish().map_err(error)?;
            publisher.reclaim().map_err(error)?;
            delivery += elapsed(started);
        }
    } else {
        monitor = Monitor::default();
        let mut connection = Connection::new(socket).map_err(error)?;
        setup = elapsed(started);
        for iteration in 0..iterations {
            let started = Instant::now();
            let source = source(mode, &process, vec![(iteration + 1) as u8; length])?;
            preparation += elapsed(started);
            let started = Instant::now();
            Transfer::tracked(source, length as u64, &monitor).serve(&mut connection);
            delivery += elapsed(started);
        }
    }
    let receiver = process.finish()?;
    let producer = monitor.snapshot();
    if receiver.bytes != (iterations * length) as u64
        || receiver.active != 0
        || producer.completed != iterations as u64
    {
        return Err("channel accounting mismatch".into());
    }
    Ok(Sample {
        mode: mode.into(),
        length,
        iterations,
        repetition,
        setup,
        preparation,
        delivery,
        throughput: (iterations * length) as f64 / 1_048_576.0 / (delivery as f64 / 1e9),
        receiver,
        producer,
    })
}
fn source(mode: &str, process: &Process, bytes: Vec<u8>) -> Result<Box<dyn std::io::Read>> {
    if mode == "memory" {
        return Ok(Box::new(Cursor::new(bytes)));
    }
    let mut draft = Draft::create(&process.root, bytes.len() as u64).map_err(error)?;
    draft.write_all(&bytes).map_err(error)?;
    Ok(Box::new(draft.seal().map_err(error)?.reader()))
}
