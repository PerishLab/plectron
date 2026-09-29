use std::sync::Arc;

pub(super) struct Order<K> {
    links: Vec<Option<Link<K>>>,
    vacant: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
}
struct Link<K> {
    key: Arc<K>,
    previous: Option<usize>,
    next: Option<usize>,
}
impl<K> Order<K> {
    pub fn new() -> Self {
        Self {
            links: Vec::new(),
            vacant: Vec::new(),
            head: None,
            tail: None,
        }
    }
    pub fn first(&self) -> Option<Arc<K>> {
        Some(self.link(self.head?).key.clone())
    }
    pub fn insert(&mut self, key: Arc<K>) -> usize {
        let link = Some(Link {
            key,
            previous: None,
            next: None,
        });
        let index = match self.vacant.pop() {
            Some(index) => {
                self.links[index] = link;
                index
            }
            None => {
                self.links.push(link);
                self.links.len() - 1
            }
        };
        self.append(index);
        index
    }
    pub fn touch(&mut self, index: usize) {
        if self.tail != Some(index) {
            self.detach(index);
            self.append(index);
        }
    }
    pub fn remove(&mut self, index: usize) {
        self.detach(index);
        self.links[index] = None;
        self.vacant.push(index);
    }
    fn link(&self, index: usize) -> &Link<K> {
        self.links[index].as_ref().expect("occupied cache link")
    }
    fn edit(&mut self, index: usize) -> &mut Link<K> {
        self.links[index].as_mut().expect("occupied cache link")
    }
    fn detach(&mut self, index: usize) {
        let link = self.link(index);
        let (previous, next) = (link.previous, link.next);
        match previous {
            Some(previous) => self.edit(previous).next = next,
            None => self.head = next,
        }
        match next {
            Some(next) => self.edit(next).previous = previous,
            None => self.tail = previous,
        }
    }
    fn append(&mut self, index: usize) {
        let tail = self.tail;
        let link = self.edit(index);
        link.previous = tail;
        link.next = None;
        match tail {
            Some(tail) => self.edit(tail).next = Some(index),
            None => self.head = Some(index),
        }
        self.tail = Some(index);
    }
}
