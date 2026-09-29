use serde::Serialize;

#[derive(Serialize)]
pub struct Report {
    pub scenario: String,
    pub policy: String,
    pub delay: u64,
    pub content: u64,
    pub preparation: u64,
    pub ready: u64,
    pub metal: Option<Attempt>,
    pub cpu: Option<Attempt>,
    pub winner: String,
    pub selected: u64,
    pub duplicate: bool,
    pub maximum: Option<u32>,
    pub unit: &'static str,
    pub endpoint: &'static str,
    pub accounting: &'static str,
}

#[derive(Clone, Serialize)]
pub struct Attempt {
    pub device: Option<u64>,
    pub start: u64,
    pub finish: u64,
    pub work: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Build {
    pub content: u64,
    pub preparation: u64,
    pub ready: u64,
}

impl Attempt {
    pub fn new(device: Option<u64>, start: u64, finish: u64) -> Self {
        Self {
            device,
            start,
            finish,
            work: finish.saturating_sub(start),
        }
    }
}

impl Report {
    pub(super) fn checked(mut self, maximum: Option<u32>) -> Self {
        self.maximum = maximum;
        self
    }
}
