use pipeline::{Graph, Node, Trace, step};

struct Split;
struct Add(u64);
struct Sum;
impl Node for Split {
    type Input = u64;
    type Output = (u64, u64);
    type Error = &'static str;
    const NAME: &'static str = "split";
    fn run(&mut self, value: u64) -> Result<Self::Output, Self::Error> {
        Ok((value, value))
    }
}
impl Node for Add {
    type Input = u64;
    type Output = u64;
    type Error = &'static str;
    const NAME: &'static str = "add";
    fn run(&mut self, value: u64) -> Result<u64, Self::Error> {
        value.checked_add(self.0).ok_or("overflow")
    }
}
impl Node for Sum {
    type Input = (u64, u64);
    type Output = u64;
    type Error = &'static str;
    const NAME: &'static str = "sum";
    fn run(&mut self, (left, right): Self::Input) -> Result<u64, Self::Error> {
        left.checked_add(right).ok_or("overflow")
    }
}

fn diamond() -> impl Graph<Input = u64, Output = u64, Error = &'static str> {
    step(Split)
        .then(step(Add(1)).pair(step(Add(2))))
        .then(step(Sum))
}

#[test]
fn topology() {
    let mut graph = diamond();
    let mut trace = Trace::default();
    assert_eq!(graph.observe(4, &mut trace), Ok(11));
    let topology = graph.topology();
    assert_eq!(trace.topology.as_ref(), Some(&topology));
    let vertices = topology.vertices();
    assert_eq!(vertices.len(), 4);
    assert!(vertices[0].incoming.is_empty());
    assert_eq!(vertices[1].incoming, [0]);
    assert_eq!(vertices[2].incoming, [0]);
    assert_eq!(vertices[3].incoming, [1, 2]);
    for (id, span) in trace.spans.iter().enumerate() {
        assert_eq!(span.index, id);
        assert_eq!(span.name, vertices[id].name);
        assert!(span.success);
    }
    assert_eq!(graph.observe(u64::MAX, &mut trace), Err("overflow"));
    assert_eq!(trace.spans.len(), 2);
    assert!(!trace.spans[1].success);
    assert_eq!(graph.observe(u64::MAX - 1, &mut trace), Err("overflow"));
    assert_eq!(trace.spans.len(), 3);
    assert!(trace.spans[1].success);
    assert!(!trace.spans[2].success);
    assert_eq!(graph.run(4), Ok(11));
}

struct Move;
impl Node for Move {
    type Input = Box<u64>;
    type Output = Box<u64>;
    type Error = ();
    const NAME: &'static str = "move";
    fn run(&mut self, input: Box<u64>) -> Result<Box<u64>, ()> {
        Ok(input)
    }
}

#[test]
fn ownership() {
    let input = Box::new(7);
    let address = std::ptr::from_ref(input.as_ref());
    let output = step(Move).then(step(Move)).run(input).unwrap();
    assert_eq!(std::ptr::from_ref(output.as_ref()), address);
}

struct Silent;
impl pipeline::Observer for Silent {
    fn node<T, E>(
        &mut self,
        _: (usize, &'static str),
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        execute()
    }
}

#[test]
fn lazy() {
    use pipeline::Observer;
    Silent.begin(|| panic!("unused topology must not be constructed"));
    assert_eq!(diamond().observe(3, &mut Silent), Ok(9));
}

#[test]
#[ignore]
fn observation() {
    for round in 0..8 {
        for modeled in [round % 2 == 0, round % 2 != 0] {
            sample(modeled);
        }
    }
}

fn sample(modeled: bool) {
    use std::{hint::black_box, time::Instant};
    let mut graph = diamond();
    let start = Instant::now();
    let mut checksum = 0;
    for index in 0..1_000_000 {
        let input = black_box(index);
        let result = if modeled {
            graph.run(input)
        } else {
            direct(input)
        };
        checksum ^= black_box(result.unwrap());
    }
    println!(
        "modeled={modeled} elapsed={:?} checksum={checksum}",
        start.elapsed()
    );
}

fn direct(input: u64) -> Result<u64, &'static str> {
    let (left, right) = Split.run(input)?;
    let left = Add(1).run(left)?;
    let right = Add(2).run(right)?;
    Sum.run((left, right))
}

#[test]
fn correlated() {
    let session = observe::Session::new(observe::Options::default());
    let mut root = session.scope("action");
    assert_eq!(diamond().run(4), Ok(11));
    root.finish(true);
    let report = session.inspect();
    assert_eq!(report.records.len(), 6);
    let graph = report
        .records
        .iter()
        .find(|r| matches!(r.kind, observe::Kind::Graph { .. }))
        .unwrap();
    let observe::Kind::Graph { vertices } = &graph.kind else {
        unreachable!()
    };
    assert_eq!(vertices[3].incoming, vec![1, 2]);
    let nodes: Vec<_> = report
        .records
        .iter()
        .filter(|r| matches!(r.kind, observe::Kind::Node { .. }))
        .collect();
    assert_eq!(nodes.len(), 4);
    assert!(nodes.iter().all(|r| r.parent == Some(graph.id)));
}
