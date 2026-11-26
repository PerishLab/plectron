use std::collections::BTreeMap;
use webengine::Fault;
use webengine::resource::{Url, address};

const MAGIC: &[u8; 4] = b"PBND";
const VERSION: u32 = 1;
const ASSETS: usize = 256;
const BUDGET: usize = 16_777_216;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub mime: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub origin: String,
    pub entry: String,
    pub assets: BTreeMap<String, Asset>,
}

impl Bundle {
    pub fn new(origin: &str, entry: &str) -> Result<Self, Fault> {
        let root = validate(origin)?;
        let target = address(entry)?;
        if authority(&target)? != root {
            return Err(Fault("bundle entry does not match bundle origin".into()));
        }
        Ok(Self {
            origin: root,
            entry: target.to_string(),
            assets: BTreeMap::new(),
        })
    }

    pub fn insert(&mut self, path: &str, mime: &str, body: Vec<u8>) -> Result<(), Fault> {
        if !path.starts_with('/') || path.contains(['\\', '\0', '?', '#']) {
            return Err(Fault(format!("invalid bundle asset path: {path}")));
        }
        if mime.is_empty() || mime.contains('\0') {
            return Err(Fault("invalid bundle asset mime".into()));
        }
        if !self.assets.contains_key(path) && self.assets.len() >= ASSETS {
            return Err(Fault("bundle asset count budget exhausted".into()));
        }
        self.assets.insert(
            path.into(),
            Asset {
                mime: mime.into(),
                body,
            },
        );
        Ok(())
    }

    pub fn get(&self, path: &str) -> Option<&Asset> {
        self.assets.get(path)
    }

    pub fn encode(&self) -> Result<Vec<u8>, Fault> {
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&VERSION.to_le_bytes());
        write(&mut output, self.origin.as_bytes())?;
        write(&mut output, self.entry.as_bytes())?;
        let count = u32::try_from(self.assets.len())
            .map_err(|_| Fault("bundle asset count exceeds u32".into()))?;
        output.extend_from_slice(&count.to_le_bytes());
        for (path, asset) in &self.assets {
            write(&mut output, path.as_bytes())?;
            write(&mut output, asset.mime.as_bytes())?;
            write(&mut output, &asset.body)?;
        }
        if output.len() > BUDGET {
            return Err(Fault("bundle byte budget exhausted".into()));
        }
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Fault> {
        if bytes.len() > BUDGET {
            return Err(Fault("bundle byte budget exhausted".into()));
        }
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.take(4)? != MAGIC {
            return Err(Fault("invalid bundle magic".into()));
        }
        if cursor.word()? != VERSION {
            return Err(Fault("unsupported bundle version".into()));
        }
        let origin = cursor.text()?;
        let entry = cursor.text()?;
        let mut bundle = Self::new(&origin, &entry)?;
        let count =
            usize::try_from(cursor.word()?).map_err(|_| Fault("bundle count overflow".into()))?;
        if count > ASSETS {
            return Err(Fault("bundle asset count budget exhausted".into()));
        }
        for _ in 0..count {
            let path = cursor.text()?;
            let mime = cursor.text()?;
            let body = cursor.slice()?.to_vec();
            bundle.insert(&path, &mime, body)?;
        }
        if cursor.offset != bytes.len() {
            return Err(Fault("trailing bytes in bundle".into()));
        }
        Ok(bundle)
    }
}

pub(crate) fn authority(url: &Url) -> Result<String, Fault> {
    if url.port().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err(Fault(
            "bundle authority excludes ports and credentials".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Fault("URL has no authority".into()))?;
    Ok(format!("{}://{host}/", url.scheme()))
}

fn validate(origin: &str) -> Result<String, Fault> {
    let url = address(origin)?;
    if matches!(url.scheme(), "http" | "https" | "file") {
        return Err(Fault("bundle origin requires a custom scheme".into()));
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(Fault("bundle origin requires an authority root".into()));
    }
    authority(&url)
}

fn write(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Fault> {
    let length =
        u32::try_from(bytes.len()).map_err(|_| Fault("bundle field length exceeds u32".into()))?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(bytes);
    if output.len() > BUDGET {
        return Err(Fault("bundle byte budget exhausted".into()));
    }
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Fault> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| Fault("bundle cursor overflow".into()))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| Fault("truncated bundle".into()))?;
        self.offset = end;
        Ok(slice)
    }

    fn word(&mut self) -> Result<u32, Fault> {
        let raw: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| Fault("truncated bundle integer".into()))?;
        Ok(u32::from_le_bytes(raw))
    }

    fn slice(&mut self) -> Result<&'a [u8], Fault> {
        let length =
            usize::try_from(self.word()?).map_err(|_| Fault("bundle length overflow".into()))?;
        self.take(length)
    }

    fn text(&mut self) -> Result<String, Fault> {
        let bytes = self.slice()?;
        std::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|error| Fault(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut bundle = Bundle::new("app://demo/", "app://demo/index.html").unwrap();
        bundle
            .insert("/style.css", "text/css", b"body {}".to_vec())
            .unwrap();
        bundle
            .insert("/index.html", "text/html", b"<html></html>".to_vec())
            .unwrap();
        let first = bundle.encode().unwrap();
        let second = bundle.encode().unwrap();
        assert_eq!(first, second);
        let decoded = Bundle::decode(&first).unwrap();
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn rejected() {
        assert!(Bundle::new("http://demo/", "http://demo/index.html").is_err());
        assert!(Bundle::new("app://demo/", "app://other/index.html").is_err());
        assert!(Bundle::decode(b"NOPE").is_err());
        let mut version = b"PBND".to_vec();
        version.extend_from_slice(&2u32.to_le_bytes());
        assert!(Bundle::decode(&version).is_err());
        let mut bundle = Bundle::new("app://demo/", "app://demo/index.html").unwrap();
        bundle
            .insert("/index.html", "text/html", b"<html></html>".to_vec())
            .unwrap();
        let mut trailing = bundle.encode().unwrap();
        trailing.push(0);
        assert!(Bundle::decode(&trailing).is_err());
    }
}
