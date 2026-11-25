use super::fixtures::{cmap, font, simple};
use std::sync::Arc;
use textengine::font::{Lease, Problem, Source};
use textengine::{Catalog, Face, Request, Shaper, Weight};

fn single(character: u16) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12];
    let delta = 1u16.wrapping_sub(character);
    for word in [
        4u16, 32, 0, 4, 0, 0, 0, character, 65535, 0, character, 65535, delta, 1, 0, 0,
    ] {
        bytes.extend(word.to_be_bytes());
    }
    bytes
}

fn pack(version: u32, members: &[Vec<u8>]) -> Vec<u8> {
    let header = 12 + members.len() * 4;
    let mut bytes = vec![0u8; header];
    bytes[0..4].copy_from_slice(b"ttcf");
    bytes[4..8].copy_from_slice(&version.to_be_bytes());
    bytes[8..12].copy_from_slice(&(members.len() as u32).to_be_bytes());
    let mut offsets = Vec::with_capacity(members.len());
    for member in members {
        let origin = bytes.len();
        offsets.push(origin as u32);
        let tables = usize::from(u16::from_be_bytes(member[4..6].try_into().unwrap()));
        let mut copy = member.clone();
        for item in 0..tables {
            let entry = 12 + item * 16;
            let start = u32::from_be_bytes(copy[entry + 8..entry + 12].try_into().unwrap());
            let shifted = start + origin as u32;
            copy[entry + 8..entry + 12].copy_from_slice(&shifted.to_be_bytes());
        }
        bytes.extend(copy);
    }
    for (index, offset) in offsets.into_iter().enumerate() {
        let slot = 12 + index * 4;
        bytes[slot..slot + 4].copy_from_slice(&offset.to_be_bytes());
    }
    bytes
}

#[test]
fn faces() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    let first = font(&[vec![], glyph.clone()], single(b'A' as u16));
    let second = font(&[vec![], glyph], single(b'B' as u16));
    for version in [0x0001_0000, 0x0002_0000] {
        let bytes: Arc<[u8]> = pack(version, &[first.clone(), second.clone()]).into();
        assert_eq!(Source::faces(&bytes), Ok(2));
        let zero = Source::face(bytes.clone(), 0).unwrap();
        assert_eq!(zero.slot(), 0);
        assert_eq!(zero.index('A').unwrap(), Some(1));
        assert_eq!(zero.index('B').unwrap(), None);
        let one = Source::face(bytes.clone(), 1).unwrap();
        assert_eq!(one.slot(), 1);
        assert_eq!(one.index('A').unwrap(), None);
        assert_eq!(one.index('B').unwrap(), Some(1));
        assert_eq!(
            Source::face(bytes.clone(), 2).unwrap_err(),
            Problem::Missing
        );
        let lease = Lease::face(bytes, 1).unwrap();
        assert_eq!(lease.source().slot(), 1);
    }
}

#[test]
fn rejected() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    let member = font(&[vec![], glyph], cmap());
    assert_eq!(Source::faces(&member), Ok(1));
    assert_eq!(
        Source::face(member.clone().into(), 1).unwrap_err(),
        Problem::Missing
    );
    let mut empty = vec![0u8; 12];
    empty[0..4].copy_from_slice(b"ttcf");
    empty[4..8].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    assert_eq!(Source::faces(&empty).unwrap_err(), Problem::Malformed);
    let mut version = pack(0x0003_0000, std::slice::from_ref(&member));
    assert_eq!(Source::faces(&version).unwrap_err(), Problem::Unsupported);
    version[4..8].copy_from_slice(&0x0002_0000u32.to_be_bytes());
    version[8..12].copy_from_slice(&65u32.to_be_bytes());
    assert_eq!(Source::faces(&version).unwrap_err(), Problem::Budget);
    let mut cff = member.clone();
    cff[0..4].copy_from_slice(b"OTTO");
    let collection: Arc<[u8]> = pack(0x0002_0000, &[cff]).into();
    assert_eq!(
        Source::face(collection, 0).unwrap_err(),
        Problem::Unsupported
    );
    let truncated: Arc<[u8]> = pack(0x0002_0000, &[member])[..14].into();
    assert_eq!(Source::faces(&truncated).unwrap_err(), Problem::Malformed);
}

struct Mock(Face);
impl textengine::Resolver for Mock {
    fn resolve(&self, family: &str, _: Weight) -> Result<Option<Face>, textengine::Fault> {
        Ok((family.eq_ignore_ascii_case("System")).then(|| self.0.clone()))
    }
}

#[test]
fn resolver() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    let bytes: Arc<[u8]> = pack(0x0002_0000, &[font(&[vec![], glyph], cmap())]).into();
    let face = Face::Owned(Lease::face(bytes, 0).unwrap());
    let catalog = Catalog::new();
    catalog.resolver(Some(Arc::new(Mock(face.clone()))));
    let request = Request {
        families: Arc::from(["System".into()]),
        size: 16.into(),
        weight: Weight::Normal,
    };
    let shaped = catalog.shape("A", &request, 0).unwrap();
    assert_eq!(shaped.spans[0].font.face, face);
    let missing = Request {
        families: Arc::from(["Absent".into()]),
        size: 16.into(),
        weight: Weight::Normal,
    };
    assert_eq!(
        catalog.shape("A", &missing, 0).unwrap().spans[0].font.face,
        Face::Missing
    );
}
