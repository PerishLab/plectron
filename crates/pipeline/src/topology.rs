#[derive(Debug, PartialEq, Eq)]
pub struct Vertex {
    pub name: &'static str,
    pub incoming: Vec<usize>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Topology {
    vertices: Vec<Vertex>,
}

impl Topology {
    pub(crate) fn append(&mut self, name: &'static str, incoming: &[usize]) -> usize {
        let id = self.vertices.len();
        self.vertices.push(Vertex {
            name,
            incoming: incoming.to_vec(),
        });
        id
    }
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }
}
