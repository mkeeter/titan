use crate::protocol::{Line, Line_};

#[derive(Debug, Eq, PartialEq)]
pub struct Document(pub Vec<Line>);

#[derive(Debug, Eq, PartialEq)]
pub struct WrappedDocument(pub Vec<Line_<Vec<String>>>);

impl Document {
    fn line_wrap(line: &Line, width: usize) -> Line_<Vec<String>> {
        use Line_::*;
        let wrapper = textwrap::Wrapper::new(width);
        let t = match line {
            Text(t) => wrapper.wrap(t),
            Link { name: Some(name), .. } => wrapper
                .initial_indent("=> ")
                .subsequent_indent("=  ")
                .wrap(name),
            Link { name: None, .. } => vec![],
            Pre { text, .. } => text.split('\n')
                .map(|s: &str| std::borrow::Cow::from(s))
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
                .initial_indent("* ")
                .subsequent_indent("  ")
                .wrap(t),
            Quote(t) => wrapper
                .initial_indent("> ")
                .subsequent_indent("> ")
                .wrap(t),
        }.into_iter()
            .map(|s| s.to_string())
            .collect();

        match line {
            Text(_) => Text(t),
            Link { name: Some(_name), url } => Link {
                name: Some(t),
                url: url.to_string() },
            Link { name: None, url } => Link { name: None, url: url.clone() },
            Pre { alt, .. } => Pre { alt: alt.clone(), text: t },
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

impl WrappedDocument {
    pub fn pretty_print(&self) {
        for block in &self.0 {
            use Line_::*;
            match block {
                H1(t) | H2(t) | H3(t) | Text(t) |
                List(t) | Quote(t) | Pre { text: t, .. } |
                Link { name: Some(t), .. } => {
                    for u in t {
                        println!("{}", u);
                    }
                    if t.is_empty() {
                        println!("");
                    }
                },
                Link { name: None, url } => {
                    println!("{}", url);
                }
                _ => unimplemented!(),
            }
        }
    }
}
