use std::borrow::Cow;
use crate::protocol::{Line, Line_};

#[derive(Debug, Eq, PartialEq)]
pub struct Document<'a>(pub Vec<Line<'a>>);

// A WrappedLine encodes a string and a flag marking whether it's the first
// in its block.
pub type WrappedLine<'a> = Line_<'a, (&'a str, bool)>;
#[derive(Debug, Eq, PartialEq)]
pub struct WrappedDocument<'a>(pub Vec<WrappedLine<'a>>);

impl Document<'_> {
    fn wrap<'a, F>(s: &'a str, width: usize, mut f: F)
        -> Box<dyn Iterator<Item=WrappedLine<'a>> + 'a>
        where F: 'a + FnMut((&'a str, bool)) -> WrappedLine<'a>
    {
        let default = f(("", true));
        let mut t = textwrap::Wrapper::new(width)
            .wrap(s)
            .into_iter()
            .map(|b: Cow<'a, str>|
                if let Cow::Borrowed(c) = b {
                    c
                } else {
                    panic!("Got unexpected owned Pre line");
                })
            .zip(std::iter::once(true).chain(std::iter::repeat(false)))
            .map(f)
            .peekable();

        if t.peek().is_some() {
            Box::new(t)
        } else {
            Box::new(std::iter::once(default))
        }
    }

    fn line_wrap<'a>(line: &'a Line, width: usize)
        -> Box<dyn Iterator<Item=WrappedLine<'a>> + 'a>
    {
        use Line_::*;
        match line {
            Text(t) => Self::wrap(t, width, Text),
            BareLink(url) => Box::new(std::iter::once(BareLink(url))),
            NamedLink { name, url } => Self::wrap(name, width - 3, move |s|
                NamedLink { url, name: s }),
            Pre { text, alt } => Box::new(text.split('\n')
                .zip(std::iter::once(true).chain(std::iter::repeat(false)))
                .map(move |(s, i)| Pre { text: (s, i), alt: *alt })),
            H1(t) => Self::wrap(t, width - 2, H1), // "# "
            H2(t) => Self::wrap(t, width - 3, H2), // "## "
            H3(t) => Self::wrap(t, width - 4, H3), // "### "
            List(t) => Self::wrap(t, width - 2, List), // "* "
            Quote(t) => Self::wrap(t, width - 2, Quote), // "> "
        }
    }
    pub fn word_wrap(&self, width: usize) -> WrappedDocument {
        WrappedDocument(self.0.iter()
            .map(|line| Self::line_wrap(line, width))
            .flatten()
            .collect()
        )
    }
}
