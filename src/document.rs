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
        let wrap = |s: &'a str, i: usize| -> Vec<&'a str> {
            let mut t: Vec<&'a str> = textwrap::Wrapper::new(width - i).wrap(s)
                .into_iter()
                .map(|b: Cow<'a, str>|
                    if let Cow::Borrowed(c) = b {
                        c
                    } else {
                        panic!("Got unexpected owned Pre line");
                    })
                .collect();

            if t.is_empty() {
                t.push("");
            }
            t
        };

        match line {
            Text(t) => Text(wrap(t, 0)),
            BareLink(url) => BareLink(url),
            NamedLink { name, url } => NamedLink {
                url,
                name: wrap(name, 3) // "=> "
            },
            Pre { text, alt } => Pre {
                text: text.clone(),
                alt: alt.clone(),
            },
            H1(t) => H1(wrap(t, 2)), // "# "
            H2(t) => H2(wrap(t, 3)), // "## "
            H3(t) => H3(wrap(t, 4)), // "### "
            List(t) => List(wrap(t, 2)), // "* "
            Quote(t) => Quote(wrap(t, 2)), // "> "
        }
    }
    pub fn word_wrap(&self, width: usize) -> WrappedDocument {
        WrappedDocument(self.0.iter()
            .map(|line| Self::line_wrap(line, width))
            .collect()
        )
    }
}
