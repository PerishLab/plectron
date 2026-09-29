use crate::Error;
use std::{ops::Index, sync::Arc};

const WIDTH: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Residency {
    pub logical: usize,
    pub owned: usize,
    pub shared: usize,
    pub pages: usize,
    pub reused: usize,
    pub width: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Traffic {
    pub materialized: usize,
    pub copied: usize,
}

#[derive(Clone)]
pub struct Store<T> {
    pages: Arc<[Arc<[T]>]>,
    length: usize,
    width: usize,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Store<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Store")
            .field("pages", &self.pages)
            .field("length", &self.length)
            .finish()
    }
}

impl<T> Store<T> {
    pub fn build(length: usize, prior: Option<&Self>) -> Result<Builder<T>, Error> {
        Self::paged(length, prior, WIDTH)
    }

    pub fn paged(length: usize, prior: Option<&Self>, width: usize) -> Result<Builder<T>, Error> {
        if width == 0 {
            return Err(Error("geometry storage page width is zero".into()));
        }
        if prior.is_some_and(|prior| prior.length != length || prior.width != width) {
            return Err(Error("geometry storage shape changed".into()));
        }
        let pages = prior.map_or_else(
            || {
                (0..length.div_ceil(width))
                    .map(|page| {
                        let length = width.min(length - page * width);
                        Page::Mutable(std::iter::repeat_with(|| None).take(length).collect())
                    })
                    .collect()
            },
            |prior| prior.pages.iter().cloned().map(Page::Shared).collect(),
        );
        Ok(Builder {
            pages,
            marked: vec![false; length],
            length,
            width,
            traffic: Traffic {
                materialized: prior.map_or(length * std::mem::size_of::<T>(), |_| 0),
                copied: 0,
            },
        })
    }

    pub const fn count(&self) -> usize {
        self.length
    }

    pub fn pages(&self) -> usize {
        self.pages.len()
    }

    pub fn get(&self, slot: usize) -> Option<&T> {
        (slot < self.length).then(|| &self.pages[slot / self.width][slot % self.width])
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.pages.iter().flat_map(|page| page.iter())
    }

    pub fn shared(&self, other: &Self) -> usize {
        self.pages
            .iter()
            .zip(other.pages.iter())
            .filter(|(left, right)| Arc::ptr_eq(left, right))
            .count()
    }

    pub fn residency(&self) -> Residency {
        let unit = std::mem::size_of::<T>();
        let table = self.pages.len() * std::mem::size_of::<Arc<[T]>>();
        let cloned = Arc::strong_count(&self.pages) > 1;
        let (mut owned, mut shared) = if cloned { (0, table) } else { (table, 0) };
        let mut reused = 0;
        for page in self.pages.iter() {
            let bytes = page.len() * unit;
            if cloned || Arc::strong_count(page) > 1 {
                shared += bytes;
                reused += 1;
            } else {
                owned += bytes;
            }
        }
        Residency {
            logical: self.length * unit,
            owned,
            shared,
            pages: self.pages.len(),
            reused,
            width: self.width,
        }
    }
}

impl<T> Index<usize> for Store<T> {
    type Output = T;

    fn index(&self, slot: usize) -> &Self::Output {
        self.get(slot).expect("geometry storage slot")
    }
}

pub struct Builder<T> {
    pages: Vec<Page<T>>,
    marked: Vec<bool>,
    length: usize,
    width: usize,
    traffic: Traffic,
}

impl<T: Clone> Builder<T> {
    pub fn write(&mut self, slot: usize, value: T) -> Result<(), Error> {
        self.mark(slot)?;
        let page = &mut self.pages[slot / self.width];
        let local = slot % self.width;
        if let Page::Shared(values) = page {
            let bytes = values.len() * std::mem::size_of::<T>();
            self.traffic.materialized += bytes;
            self.traffic.copied += bytes;
            *page = Page::Mutable(values.iter().cloned().map(Some).collect());
        }
        let Page::Mutable(values) = page else {
            unreachable!()
        };
        values[local] = Some(value);
        Ok(())
    }
}

impl<T> Builder<T> {
    pub const fn traffic(&self) -> Traffic {
        self.traffic
    }
    pub fn marked(&self, slot: usize) -> bool {
        self.marked.get(slot).copied().unwrap_or(false)
    }

    pub fn retain(&mut self, slot: usize) -> Result<(), Error> {
        self.mark(slot)?;
        let present = match &self.pages[slot / self.width] {
            Page::Shared(values) => values.get(slot % self.width).is_some(),
            Page::Mutable(values) => values.get(slot % self.width).is_some_and(Option::is_some),
        };
        if !present {
            return Err(Error("geometry storage retained an absent state".into()));
        }
        Ok(())
    }

    pub fn finish(self) -> Result<Store<T>, Error> {
        if self.marked.iter().any(|marked| !marked) {
            return Err(Error("geometry storage is incomplete".into()));
        }
        let pages = finish(self.pages)?;
        Ok(Store {
            pages: pages.into(),
            length: self.length,
            width: self.width,
        })
    }

    fn mark(&mut self, slot: usize) -> Result<(), Error> {
        let marked = self
            .marked
            .get_mut(slot)
            .ok_or_else(|| Error("geometry storage slot is outside its generation".into()))?;
        if std::mem::replace(marked, true) {
            return Err(Error("geometry storage contains duplicate state".into()));
        }
        Ok(())
    }
}

enum Page<T> {
    Shared(Arc<[T]>),
    Mutable(Vec<Option<T>>),
}

fn finish<T>(pages: Vec<Page<T>>) -> Result<Vec<Arc<[T]>>, Error> {
    pages
        .into_iter()
        .map(|page| match page {
            Page::Shared(values) => Ok(values),
            Page::Mutable(values) => values
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(Arc::from)
                .ok_or_else(|| Error("geometry storage is incomplete".into())),
        })
        .collect()
}
