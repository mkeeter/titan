use std::borrow::Cow;
use crate::protocol::{Line, Line_};

#[derive(Debug, Eq, PartialEq)]
pub struct Document<'a>(pub Vec<Line<'a>>);

pub type WrappedLine<'a> = Line_<'a, Vec<&'a str>>;
#[derive(Debug, Eq, PartialEq)]
pub struct WrappedDocument<'a>(pub Vec<WrappedLine<'a>>);

impl Document<'_> {
    fn line_wrap<'a>(line: &'a Line, width: usize) -> WrappedLine<'a>
    {
        use Line_::*;
        let w = textwrap::Wrapper::new;
        let t = match line {
            Text(t) => w(width).wrap(t),
            Link { name: Some(name), .. } => w(width - 3).wrap(name), // "=> "
            Link { name: None, .. } => vec![],
            Pre { text, .. } => text.iter()
                .map(|s| std::borrow::Cow::from(*s))
                .collect(),
            H1(t) => w(width - 2).wrap(t), // "# "
            H2(t) => w(width - 3).wrap(t), // "## "
            H3(t) => w(width - 4).wrap(t), // "### "
            List(t) => w(width - 2).wrap(t), // "* "
            Quote(t) => w(width - 2).wrap(t), // "> "
        }.into_iter()
            .map(|b: Cow<'a, str>|
                if let Cow::Borrowed(c) = b {
                    c
                } else {
                    panic!("Got unexpected owned Pre line");
                })
            .collect();

        match line {
            Text(_) => Text(t),
            Link { name: Some(_name), url } => Link {
                name: Some(t),
                url },
            Link { name: None, url } => Link { name: None, url },
            Pre { alt, .. } => Pre { alt: *alt, text: t },
            H1(_) => H1(t),
            H2(_) => H2(t),
            H3(_) => H3(t),
            List(_) => List(t),
            Quote(_) => Quote(t),
        }
    }
    pub fn word_wrap(&self, width: usize) -> WrappedDocument {
        WrappedDocument(self.0.iter()
            .map(|line| Self::line_wrap(line, width))
            .collect()
        )
    }
}
