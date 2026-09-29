use std::sync::Arc;
use textengine::font::Source;

pub fn words(values: &[u16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}
pub fn link(parent: &mut Vec<u8>, offset: usize, child: Vec<u8>) {
    let start = u16::try_from(parent.len()).unwrap();
    parent[offset..offset + 2].copy_from_slice(&start.to_be_bytes());
    parent.extend(child);
}
pub fn source(tag: &[u8; 4], table: Vec<u8>) -> Source {
    resource(vec![(tag, table)])
}
pub fn resource(tables: Vec<(&[u8; 4], Vec<u8>)>) -> Source {
    let original = ::fixtures::bytes("Lato-Regular.ttf");
    let count = u16::from_be_bytes(original[4..6].try_into().unwrap());
    let mut entries = std::collections::BTreeMap::<[u8; 4], Vec<u8>>::new();
    for index in 0..usize::from(count) {
        let offset = 12 + index * 16;
        let tag = original[offset..offset + 4].try_into().unwrap();
        let start =
            u32::from_be_bytes(original[offset + 8..offset + 12].try_into().unwrap()) as usize;
        let size =
            u32::from_be_bytes(original[offset + 12..offset + 16].try_into().unwrap()) as usize;
        entries.insert(tag, original[start..start + size].to_vec());
    }
    for (tag, table) in tables {
        entries.insert(*tag, table);
    }
    let mut bytes = original[..12].to_vec();
    bytes[4..6].copy_from_slice(&(entries.len() as u16).to_be_bytes());
    bytes.resize(12 + entries.len() * 16, 0);
    for (index, (tag, table)) in entries.into_iter().enumerate() {
        let offset = 12 + index * 16;
        let start = bytes.len() as u32;
        bytes[offset..offset + 4].copy_from_slice(&tag);
        bytes[offset + 8..offset + 12].copy_from_slice(&start.to_be_bytes());
        bytes[offset + 12..offset + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        bytes.extend(table);
        bytes.resize(bytes.len().next_multiple_of(4), 0);
    }
    Source::parse(Arc::from(bytes)).unwrap()
}
pub fn layout(feature: &[u8; 4], lookups: Vec<Vec<u8>>) -> Vec<u8> {
    let mut script = words(&[1]);
    script.extend(b"DFLT");
    script.extend(words(&[8, 4, 0, 0, 0xffff, 1, 0]));
    let mut features = words(&[1]);
    features.extend(feature);
    features.extend(words(&[8, 0, lookups.len() as u16]));
    features.extend(words(&(0..lookups.len() as u16).collect::<Vec<_>>()));
    let mut list = words(&[lookups.len() as u16]);
    list.resize(2 + lookups.len() * 2, 0);
    for (index, lookup) in lookups.into_iter().enumerate() {
        link(&mut list, 2 + index * 2, lookup);
    }
    let mut data = words(&[1, 0, 0, 0, 0]);
    link(&mut data, 4, script);
    link(&mut data, 6, features);
    link(&mut data, 8, list);
    data
}
pub fn lookup(kind: u16, flags: u16, subtable: Vec<u8>) -> Vec<u8> {
    let mut data = words(&[kind, flags, 1, 8]);
    data.extend(subtable);
    data
}
pub fn extension(kind: u16, subtable: Vec<u8>) -> Vec<u8> {
    let mut data = words(&[1, kind, 0, 8]);
    data.extend(subtable);
    data
}
pub fn single(from: u16, to: u16) -> Vec<u8> {
    let mut data = words(&[2, 0, 1, to]);
    link(&mut data, 2, words(&[1, 1, from]));
    data
}
pub fn ligature(input: [u16; 2], output: u16) -> Vec<u8> {
    let mut set = words(&[1, 4]);
    set.extend(words(&[output, 2, input[1]]));
    let mut data = words(&[1, 0, 1, 0]);
    link(&mut data, 6, set);
    link(&mut data, 2, words(&[2, 1, input[0], input[0], 0]));
    data
}
pub fn mark(base: u16, mark: u16, anchors: [[i16; 2]; 2]) -> Vec<u8> {
    let mut marks = words(&[1, 0, 6]);
    marks.extend(words(&[1, anchors[1][0] as u16, anchors[1][1] as u16]));
    let mut bases = words(&[1, 4]);
    bases.extend(words(&[1, anchors[0][0] as u16, anchors[0][1] as u16]));
    let mut data = words(&[1, 0, 0, 1, 0, 0]);
    link(&mut data, 2, words(&[1, 1, mark]));
    link(&mut data, 4, words(&[1, 1, base]));
    link(&mut data, 8, marks);
    link(&mut data, 10, bases);
    data
}
