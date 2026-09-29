use domengine::style::animation::{Schedule, Track};

#[derive(Default)]
pub(crate) struct Animator {
    generation: u64,
    tracks: Vec<Track<u64>>,
}

impl Animator {
    pub fn replace(&mut self, generation: u64, tracks: Vec<Track<u64>>) {
        self.generation = generation;
        self.tracks = tracks;
    }
    pub fn schedule(&self, now: u64) -> Option<Schedule> {
        let mut deadline = None;
        for track in &self.tracks {
            match track.plan.schedule(now) {
                Some(Schedule::Continuous) => return Some(Schedule::Continuous),
                Some(Schedule::At(at)) => {
                    deadline = Some(deadline.map_or(at, |current: u64| current.min(at)));
                }
                None => (),
            }
        }
        deadline.map(Schedule::At)
    }
    pub fn advance(
        &mut self,
        now: u64,
    ) -> Result<Option<graphics::shared::Effects>, graphics::shared::Fault> {
        let values = self
            .tracks
            .iter_mut()
            .filter_map(|track| {
                track
                    .plan
                    .advance(now)
                    .map(|opacity| graphics::shared::Effect {
                        space: track.target,
                        opacity,
                    })
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            Ok(None)
        } else {
            graphics::shared::Effects::new(self.generation, values).map(Some)
        }
    }
}
