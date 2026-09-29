use super::Content;

impl<F, R, C> Content<F, R, C> {
    pub fn geometry<D>(self, mut map: impl FnMut(C) -> D) -> Content<F, R, D> {
        match self {
            Self::Image { raster } => Content::Image { raster },
            Self::Box {
                color,
                translation,
                opacity,
                viewport,
                scroll,
            } => Content::Box {
                color,
                translation,
                opacity,
                viewport: viewport.map(&mut map),
                scroll,
            },
            Self::Line => Content::Line,
            Self::Inline { translation } => Content::Inline { translation },
            Self::Selection { color } => Content::Selection { color },
            Self::Caret { color } => Content::Caret { color },
            Self::Text {
                font,
                glyphs,
                clusters,
                clip,
                color,
            } => Content::Text {
                font,
                glyphs,
                clusters,
                clip: clip.map(map),
                color,
            },
        }
    }
}
