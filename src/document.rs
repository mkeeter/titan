use std::borrow::Cow;
use crate::protocol::Line;

#[derive(Debug, Eq, PartialEq)]
pub struct Document_<T>(pub Vec<T>);

// This is our as-parsed document, with long lines
pub type Document<'a> = Document_<Line<'a>>;

// WrappedDocument encodes a set of screen-wrapped lines, each with a flag
// indicating whether it's the first line in its block.  This matters for
// rendering, e.g. a list shows "• " on the first line of each item.
pub type WrappedDocument<'a> = Document_<(Line<'a>, bool)>;

impl<T> Document_<T> {
    pub fn new(t: Vec<T>) -> Self {
        Self(t)
    }
}

impl Document<'_> {
    fn wrap<'a, F>(s: &'a str, width: usize, mut f: F)
        -> Box<dyn Iterator<Item=(Line<'a>, bool)> + 'a>
        where F: 'a + FnMut(&'a str) -> Line<'a>
    {
        let default = f("");
        let mut t = textwrap::Wrapper::new(width)
            .wrap(s)
            .into_iter()
            .map(|b: Cow<'a, str>|
                if let Cow::Borrowed(c) = b {
                    c
                } else {
                    panic!("Got unexpected owned Pre line");
                })
            .map(f)
            .zip(std::iter::once(true).chain(std::iter::repeat(false)))
            .peekable();

        if t.peek().is_some() {
            Box::new(t)
        } else {
            Box::new(std::iter::once((default, true)))
        }
    }

    fn line_wrap<'a>(line: &'a Line, width: usize)
        -> Box<dyn Iterator<Item=(Line<'a>, bool)> + 'a>
    {
        use Line::*;
        match line {
            Text(t) => Self::wrap(t, width, Text),
            BareLink(url) => Box::new(std::iter::once((BareLink(url), true))),
            NamedLink { name, url } => Self::wrap(name, width - 3, move |s|
                NamedLink { url, name: s }),
            Pre { text, alt } => Box::new(text.split('\n')
                .map(move |s| Pre { text: s, alt: *alt })
                .zip(std::iter::once(true).chain(std::iter::repeat(false)))),
            H1(t) => Self::wrap(t, width - 2, H1), // "# "
            H2(t) => Self::wrap(t, width - 3, H2), // "## "
            H3(t) => Self::wrap(t, width - 4, H3), // "### "
            List(t) => Self::wrap(t, width - 2, List), // "* "
            Quote(t) => Self::wrap(t, width - 2, Quote), // "> "
        }
    }
    pub fn word_wrap(&self, width: usize) -> WrappedDocument {
        WrappedDocument::new(self.0.iter()
            .map(|line| Self::line_wrap(line, width))
            .flatten()
            .collect()
        )
    }

    pub fn dummy_wrap(&self) -> WrappedDocument {
        WrappedDocument::new(self.0.iter()
            .map(|line| (*line, true))
            .collect())
    }
}
