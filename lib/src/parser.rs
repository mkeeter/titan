use std::convert::TryFrom;

use crate::document::Document;
use crate::Error;

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag, take_while_m_n, take_until, take_till},
    character::{is_digit},
    character::complete::space0,
    combinator::map_res,
    sequence::{terminated, tuple},
};

use crate::protocol::{Status, Response, Line};

// Temporary tuple type, to make nom's type-inference happy
type ResponseHeader<'a> = (Status, &'a str);

pub fn parse_response_header(input: &[u8]) -> IResult<&[u8], ResponseHeader> {
    let (input, (status, _, meta)) = tuple((
        map_res(
            take_while_m_n(2, 2, is_digit),
            |i| {
                let s = std::str::from_utf8(i)
                    .expect("Could not convert to utf8");
                let n = u32::from_str_radix(s, 10)
                    .expect("Could not get u32");
                Status::try_from(n)
            }),
        tag(" "),
        map_res(
            terminated(take_while_m_n(0, 1024, |c: u8| c != b'\r'),
                       tag("\r\n")),
            std::str::from_utf8)
    ))(input)?;

    Ok((input, (status, meta)))
}

pub fn parse_response(input: &[u8]) -> Result<Response, Error> {
    let (body, (status, meta)) = parse_response_header(input)
        .map_err(|_| Error::ParseError)?;
    Ok(Response { status, meta, body })
}

////////////////////////////////////////////////////////////////////////////////

/// Reads a single line up until the newline, consuming the terminator
fn read_line(input: &str) -> IResult<&str, &str> {
    terminated(alt((is_not("\r\n"), tag(""))),
               alt((tag("\r\n"), tag("\n"), tag(""))))(input)

}

fn read_prefixed<'a, F>(input: &'a str, t: &'static str, f: F)
    -> IResult<&'a str, Line<'a>>
    where F: Fn(&str) -> Line
{
    let (input, (_, _, o)) = tuple((tag(t), space0, read_line))(input)?;
    Ok((input, f(o)))
}

fn parse_line_h1(input: &str) -> IResult<&str, Line> {
    read_prefixed(input, "#", |s| Line::H1(s))
}

fn parse_line_h2(input: &str) -> IResult<&str, Line> {
    read_prefixed(input, "##", |s| Line::H2(s))
}

fn parse_line_h3(input: &str) -> IResult<&str, Line> {
    read_prefixed(input, "###", |s| Line::H3(s))
}

fn parse_line_list(input: &str) -> IResult<&str, Line> {
    read_prefixed(input, "* ", |s| Line::List(s))
}

fn parse_line_quote(input: &str) -> IResult<&str, Line> {
    read_prefixed(input, ">", |s| Line::Quote(s))
}

fn parse_line_link(input: &str) -> IResult<&str, Line> {
    let (input, (_, url, name)) = tuple((
            terminated(tag("=>"), space0),
            terminated(take_till(char::is_whitespace), space0),
            read_line))(input)?;

    Ok((input,
        if name.is_empty() {
            Line::BareLink(url)
        } else {
            Line::NamedLink { url, name }
        }))
}

fn parse_pre(input: &str) -> IResult<&str, Line> {
    let (input, (_, alt)) = tuple((tag("```"), read_line))(input)?;
    let alt = if alt.is_empty() {
        None
    } else {
        Some(alt)
    };
    let (input, text) = take_until("\n```\n")(input)?;
    let (input, _) = tag("\n```\n")(input)?;

    Ok((input, Line::Pre { alt, text }))
}

fn parse_line_text(input: &str) -> IResult<&str, Line> {
    let (input, text) = read_line(input)?;
    Ok((input, Line::Text(text)))
}

/// Parse a single line or preformatted block of text/gemini
fn parse_line(input: &str) -> IResult<&str, Line> {
    alt((parse_line_h3, parse_line_h2, parse_line_h1, parse_line_list,
         parse_line_quote, parse_line_link, parse_pre, parse_line_text))
        (input)
}

/// Parse a full text/gemini document
pub fn parse_text_gemini(mut input: &str) -> IResult<&str, Document> {
    let mut out = Vec::new();

    while !input.is_empty() {
        let (input_, parsed) = parse_line(input)?;
        input = input_;
        out.push(parsed);
    }

    Ok((input, Document(out)))
}

#[test]
pub fn test_parse_text_gemini() {
    let r = parse_text_gemini("# h1
> quote
## h2

```py
for i in range(10):
    print(i)
```
hi there").unwrap();
    assert_eq!(r.1, Document::new(vec![
        Line::H1("h1"),
        Line::Quote("quote"),
        Line::H2("h2"),
        Line::Text(""),
        Line::Pre { alt: Some("py"), text:
            "for i in range(10):\n    print(i)"},
        Line::Text("hi there"),
    ]));
}

#[test]
pub fn test_parse_line() {
    let r = parse_line("=> hello.com world").unwrap();
    assert_eq!(r.1, Line::NamedLink {
        url: "hello.com",
        name: "world" });

    let r = parse_line("=> hello.com ").unwrap();
    assert_eq!(r.1, Line::BareLink("hello.com"));

    let r = parse_line("#header").unwrap();
    assert_eq!(r.1, Line::H1("header"));

    let r = parse_line("#  header").unwrap();
    assert_eq!(r.1, Line::H1("header"));

    let r = parse_line("> quote").unwrap();
    assert_eq!(r.1, Line::Quote("quote"));
}
