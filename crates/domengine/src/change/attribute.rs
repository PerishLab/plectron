use crate::Fault;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    Id,
    Class,
    Style,
    Dir,
    Src,
    Href,
    Rel,
}
impl Attribute {
    pub(crate) fn parse(name: &str) -> Result<Self, Fault> {
        match name {
            "id" => Ok(Self::Id),
            "class" => Ok(Self::Class),
            "style" => Ok(Self::Style),
            "dir" => Ok(Self::Dir),
            "src" => Ok(Self::Src),
            "href" => Ok(Self::Href),
            "rel" => Ok(Self::Rel),
            _ => Err(Fault(format!("unsupported attribute: {name}"))),
        }
    }
}
