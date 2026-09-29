use super::{Measure, Measured};
use crate::layout::input::Entry;
use crate::layout::{Constraints, Shaper};
use crate::style::Display;

impl<S: Shaper> Measure<'_, S> {
    pub(in crate::layout) fn record(
        &mut self,
        element: Entry<'_>,
        constraints: Constraints,
        state: (crate::layout::cache::Activity, usize),
    ) {
        let category = if element.picture.is_some() {
            crate::layout::cache::Category::Image
        } else if element.key.text.is_some() {
            crate::layout::cache::Category::Text
        } else if element.key.style.display == Display::Flex {
            crate::layout::cache::Category::Flex
        } else {
            crate::layout::cache::Category::Block
        };
        if let Some(causality) = &mut self.causality {
            causality.record(element, constraints, (state.0, category), state.1);
        }
    }

    pub(in crate::layout) fn admission(
        &mut self,
        element: Entry<'_>,
        constraints: Constraints,
        disposition: crate::layout::cache::Disposition,
        work: usize,
    ) {
        use crate::layout::cache::{Activity, Disposition};
        let activity = match disposition {
            Disposition::Disabled => None,
            Disposition::Bypass => Some(Activity::Bypass),
            Disposition::Screen => Some(Activity::Screen),
            Disposition::Store => Some(Activity::Store),
        };
        if let Some(activity) = activity {
            self.record(element, constraints, (activity, work));
        }
    }

    pub(in crate::layout) fn reuse<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
        lookup: crate::layout::cache::Lookup<'a>,
    ) -> Option<Measured<'a>> {
        match lookup {
            crate::layout::cache::Lookup::Hit(measured, windowed) => {
                self.record(
                    element,
                    constraints,
                    (crate::layout::cache::Activity::Hit, measured.work),
                );
                if windowed {
                    self.record(
                        element,
                        constraints,
                        (crate::layout::cache::Activity::Window, measured.work),
                    );
                }
                Some(measured)
            }
            crate::layout::cache::Lookup::Miss(cause) => {
                self.record(
                    element,
                    constraints,
                    (crate::layout::cache::Activity::Miss, 0),
                );
                self.record(element, constraints, (cause, 0));
                None
            }
        }
    }
}
