use std::convert::TryFrom;

use anyhow::Result;

use crate::document::Document;

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag, take_while_m_n, take_till},
    character::{is_digit},
    character::complete::space0,
    combinator::map_res,
    sequence::{terminated, tuple},
};

use crate::protocol::{ResponseStatus, ResponseHeader, Line};

fn parse_response_status(i: &[u8]) -> Result<ResponseStatus> {
    let s = std::str::from_utf8(i)?;
    ResponseStatus::try_from(u32::from_str_radix(s, 10)?)
}

pub fn parse_response_header(input: &[u8]) -> IResult<&[u8], ResponseHeader> {
    let (input, (status, _, meta)) = tuple((
        map_res(
            take_while_m_n(2, 2, is_digit),
            parse_response_status),
        tag(" "),
        map_res(
            terminated(take_while_m_n(0, 1024, |c: u8| c != b'\r'),
                       tag("\r\n")),
            std::str::from_utf8)
    ))(input)?;

    Ok((input, ResponseHeader { status, meta: meta }))
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

    let url = url;
    let name = if name.is_empty() {
        None
    } else {
        Some(name)
    };
    Ok((input, Line::Link { url, name }))
}

fn parse_line_pre(input: &str) -> IResult<&str, Line> {
    let (input, (_, alt)) = tuple((tag("```"), read_line))(input)?;
    let alt = if alt.is_empty() {
        None
    } else {
        Some(alt)
    };
    Ok((input, Line::Pre { alt, text: "" }))
}

fn parse_line_text(input: &str) -> IResult<&str, Line> {
    let (input, text) = read_line(input)?;
    Ok((input, Line::Text(text)))
}

/// Parse a single line of text/gemini
pub fn parse_line(input: &str) -> IResult<&str, Line> {
    alt((parse_line_h3, parse_line_h2, parse_line_h1, parse_line_list,
         parse_line_quote, parse_line_link, parse_line_pre, parse_line_text))
        (input)
}

/// Parse a full text/gemini document
pub fn parse_text_gemini<'a>(mut input: &'a str) -> IResult<&str, Document> {
    let mut out = Vec::new();

    // This struct lets us accumulate a whole block of preformatted text,
    // rather than having an accidentally quadratic accumulator of lines.
    struct PreArray<'a> {
        lines: Vec<&'a str>,
        alt: Option<&'a str>,
    }
    let mut in_pre: Option<PreArray<'a>> = None;

    while !input.is_empty() {
        // If we're in the middle of a preformatted block, then check to see
        // whether this line ends the block; otherwise, accumulate raw text
        /*
        if let Some(pre) = in_pre.as_mut() {
            let r = parse_line_pre(input);
            if let Ok((input_, _alt)) = r {
                out.push(Line::Pre {
                    alt: pre.alt.take(),
                    text: pre.lines.join("\n") });
                in_pre = None;
                input = input_;
            } else {
                let (input_, line) = read_line(input)?;
                pre.lines.push(line);
                input = input_;
            }
        } else {
            */
            let (input_, parsed) = parse_line(input)?;
            input = input_;
            if let Line::Pre { alt, .. } = parsed {
                in_pre = Some(PreArray { lines: Vec::new(), alt });
            } else {
                out.push(parsed);
            }
        //}
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
```").unwrap();
    assert_eq!(r.1, Document(vec![
        Line::H1("h1"),
        Line::Quote("quote"),
        Line::H2("h2"),
        Line::Text(""),
        Line::Pre { alt: Some("py"), text: "for i in range(10):
    print(i)"},
    ]));
}

#[test]
pub fn test_parse_line() {
    let r = parse_line("=> hello.com world").unwrap();
    assert_eq!(r.1, Line::Link {
        url: "hello.com",
        name: Some("world") });

    let r = parse_line("=> hello.com ").unwrap();
    assert_eq!(r.1, Line::Link {
        url: "hello.com",
        name: None });

    let r = parse_line("#header").unwrap();
    assert_eq!(r.1, Line::H1("header"));

    let r = parse_line("#  header").unwrap();
    assert_eq!(r.1, Line::H1("header"));

    let r = parse_line("> quote").unwrap();
    assert_eq!(r.1, Line::Quote("quote"));

    let r = parse_line("```py").unwrap();
    assert_eq!(r.1, Line::Pre {
        alt: Some("py"),
        text: "" });
}
