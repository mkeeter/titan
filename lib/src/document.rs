use crate::protocol::Line;

#[derive(Debug, Eq, PartialEq)]
pub struct Document<'a>(pub Vec<Line<'a>>);
