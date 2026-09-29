use super::Lines;
use graphics::shared::Scalar;
impl super::Paragraph {
    pub(in crate::layout) fn routed(&self, retained: bool) {
        self.trace.route(retained);
    }
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = self.prepared.cost();
        cost.extend(&self.sequence.cost());
        cost.bytes += std::mem::size_of::<Self>()
            + self.source.len()
            + self.style.font.retained()
            + std::mem::size_of_val(self.kinds.as_ref())
            + std::mem::size_of_val(self.opportunities.as_ref())
            + std::mem::size_of_val(self.bidi.units.as_slice())
            + std::mem::size_of_val(self.bidi.paragraphs.as_slice());
        cost
    }
}
impl Lines {
    pub(in crate::layout) const fn attachment(&self) -> geoengine::Attachment {
        self.attachment
    }

    pub(in crate::layout) fn same(&self, other: &Self) -> bool {
        (self.descent, self.height, self.baseline, &self.issues)
            == (other.descent, other.height, other.baseline, &other.issues)
            && self.ranges.len() == other.ranges.len()
            && self.ranges.iter().zip(&other.ranges).all(|(a, b)| {
                (a.direction, &a.source, a.height, a.baseline, a.y, a.width)
                    == (b.direction, &b.source, b.height, b.baseline, b.y, b.width)
                    && a.visual == b.visual
                    && a.sequence.same(&b.sequence)
            })
    }
    pub(super) fn position(mut self) -> Self {
        let mut y = 0;
        for line in &mut self.ranges {
            let atom = (0..line.sequence.units.len())
                .filter_map(|index| line.sequence.atom(index))
                .map(|(_, size)| Scalar::from(size[1] as i64))
                .max();
            let ascent = atom.map_or(self.baseline, |height| height.max(self.baseline));
            let descent = Scalar::from(self.height as i64) - self.baseline;
            let descent = if atom.is_some() {
                descent.max(Scalar::ZERO)
            } else {
                descent
            };
            line.baseline = ascent;
            line.height = (ascent + descent).ceil();
            line.y = y;
            y += line.height;
        }
        self
    }
    pub(in crate::layout) fn extent(&self) -> usize {
        self.ranges.last().map_or(0, |line| line.y + line.height)
    }
    pub(in crate::layout) fn ink(&self) -> usize {
        self.ranges
            .iter()
            .map(|line| (Scalar::from(line.y as i64) + line.baseline + self.descent).ceil())
            .max()
            .unwrap_or(0)
    }
}

impl Lines {
    pub(in crate::layout) fn cost(&self) -> &shared::cache::Cost {
        self.accounting.get_or_init(|| {
            let mut cost = shared::cache::Cost::new(
                std::mem::size_of::<Self>()
                    + self.issues.len() * std::mem::size_of::<textengine::Issue>()
                    + self.ranges.len() * std::mem::size_of::<super::Line>(),
            );
            for line in &self.ranges {
                cost.extend(&line.sequence.cost());
                cost.bytes += line.visual.len() * std::mem::size_of::<super::Placed>();
            }
            cost.share()
        })
    }
}
