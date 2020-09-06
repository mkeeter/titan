use std::borrow::Cow;
use crate::protocol::{Line, Line_};

#[derive(Debug, Eq, PartialEq)]
pub struct Document<'a>(pub Vec<Line<'a>>);

#[derive(Debug, Eq, PartialEq)]
pub struct WrappedDocument<'a>(pub Vec<Line_<'a, Vec<Cow<'a, str>>>>);

impl Document<'_> {
    fn line_wrap<'a>(line: &'a Line, width: usize)
        -> Line_<'a, Vec<Cow<'a, str>>>
    {
        use Line_::*;
        let wrapper = textwrap::Wrapper::new(width);
        let t = match line {
            Text(t) => wrapper.wrap(t),
            Link { name: Some(name), .. } => wrapper
                .initial_indent("=> ")
                .subsequent_indent("   ")
                .wrap(name),
            Link { name: None, .. } => vec![],
            Pre { text, .. } => text.iter()
                .map(|s| std::borrow::Cow::from(*s))
                .collect(),
            H1(t) => wrapper
                .initial_indent("# ")
                .subsequent_indent("# ")
                .wrap(t),
            H2(t) => wrapper
                .initial_indent("## ")
                .subsequent_indent("   ")
                .wrap(t),
            H3(t) => wrapper
                .initial_indent("### ")
                .subsequent_indent("    ")
                .wrap(t),
            List(t) => wrapper
                .initial_indent("• ")
                .subsequent_indent("  ")
                .wrap(t),
            Quote(t) => wrapper
                .initial_indent("> ")
                .subsequent_indent("> ")
                .wrap(t),
        }.into_iter()
            .collect();

        match line {
            Text(_) => Text(t),
            Link { name: Some(_name), url } => Link {
                name: Some(t),
                url },
            Link { name: None, url } => Link { name: None, url },
            Pre { alt, .. } => Pre { alt: *alt, text: t.iter().map(|c|
                if let Cow::Borrowed(c) = c {
                    *c
                } else {
                    panic!("Got unexpected owned Pre line");
                }).collect()
            },
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

impl WrappedDocument<'_> {
    pub fn pretty_print(&self) {
        for block in &self.0 {
            use Line_::*;
            use colored::*;
            let color_fn = |s: &str| match block {
                H1(_) =>  s.color("red"),
                H2(_) =>  s.color("yellow"),
                H3(_) =>  s.color("green"),
                Text(_) =>  s.clear(),
                Quote(_) =>  s.color("cyan"),
                Pre { .. } =>  s.color("orange"),
                Link { .. } => s.color("magenta"),
                List(_) => s.clear(),
            };
            match block {
                H1(t) | H2(t) | H3(t) | Text(t) |
                List(t) | Quote(t) |
                Link { name: Some(t), .. } => {
                    for u in t {
                        println!("{}", color_fn(u));
                    }
                    if t.is_empty() {
                        println!();
                    }
                },
                Pre { text: t, .. } => {
                    for u in t {
                        println!("{}", color_fn(u));
                    }
                },
                Link { name: None, url } => {
                    println!("{}", url);
                }
            }
        }
    }
}
