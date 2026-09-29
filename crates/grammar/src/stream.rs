pub struct Stream<T> {
    items: Vec<T>,
    end: T,
    position: usize,
}

impl<T> Stream<T> {
    pub fn new(items: Vec<T>, end: T) -> Self {
        Self {
            items,
            end,
            position: 0,
        }
    }

    pub fn peek(&self) -> &T {
        self.look(0)
    }

    pub fn look(&self, distance: usize) -> &T {
        self.items
            .get(self.position.saturating_add(distance))
            .unwrap_or(&self.end)
    }

    pub fn previous(&self) -> Option<&T> {
        self.position
            .checked_sub(1)
            .and_then(|index| self.items.get(index))
    }

    pub fn take(&mut self) -> &T {
        let index = self.position;
        if index < self.items.len() {
            self.position += 1;
        }
        self.items.get(index).unwrap_or(&self.end)
    }

    pub fn eat(&mut self, predicate: impl FnOnce(&T) -> bool) -> bool {
        if self.position == self.items.len() || !predicate(self.peek()) {
            return false;
        }
        self.position += 1;
        true
    }
}
