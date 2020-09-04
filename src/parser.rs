use std::convert::TryFrom;

use anyhow::Result;

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while_m_n, take_while},
    character::{is_digit},
    combinator::{map_res, rest},
    sequence::terminated,
};

use crate::protocol::{ResponseStatus, ResponseHeader, Line};

fn parse_status(i: &[u8]) -> Result<ResponseStatus> {
    let s = std::str::from_utf8(i)?;
    ResponseStatus::try_from(u32::from_str_radix(s, 10)?)
}

pub fn parse_header(input: &[u8]) -> IResult<&[u8], ResponseHeader> {
    let (input, status) = map_res(
        take_while_m_n(2, 2, is_digit),
        parse_status
    )(input)?;

    let (input, _) = tag(" ")(input)?;
    let (input, meta) = map_res(
        take_while_m_n(0, 1024, |c: u8| c != b'\r'),
        std::str::from_utf8
    )(input)?;
    let (input, _) = tag("\r\n")(input)?;

    let meta = meta.to_string();

    Ok((input, ResponseHeader { status, meta }))
}

////////////////////////////////////////////////////////////////////////////////

pub fn parse_line_h1(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("#")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    Ok((input, Line::H1(input.to_string())))
}

pub fn parse_line_h2(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("##")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    Ok((input, Line::H2(input.to_string())))
}

pub fn parse_line_h3(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("###")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    Ok((input, Line::H3(input.to_string())))
}

pub fn parse_line_list(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("* ")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    Ok((input, Line::List(input.to_string())))
}

pub fn parse_line_quote(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag(">")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    Ok((input, Line::Quote(input.to_string())))
}

pub fn parse_line_link(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("=>")(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;
    let (input, url) = take_while(|c: char| !c.is_whitespace())(input)?;
    let (input, _) = take_while(char::is_whitespace)(input)?;

    let url = url.to_string();
    let name = if input.is_empty() {
        None
    } else {
        Some(input.to_string())
    };
    Ok((input, Line::Link { url, name }))
}

pub fn parse_line_pre(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag("```")(input)?;
    let (input, alt) = rest(input)?;
    let alt = if alt.is_empty() {
        None
    } else {
        Some(alt.to_string())
    };
    Ok((input, Line::Pre { alt, text: String::new() }))
}

pub fn parse_line_text(input: &str) -> IResult<&str, Line> {
    let (input, text) = rest(input)?;
    Ok((input, Line::Text(text.to_string())))
}

pub fn parse_line(input: &str) -> IResult<&str, Line> {
    alt((parse_line_h3, parse_line_h2, parse_line_h1, parse_line_list,
         parse_line_quote, parse_line_link, parse_line_pre, parse_line_text))
        (input)
}

pub fn parse_text_gemini(mut input: &str) -> IResult<&str, Vec<Line>> {
    let mut out = Vec::new();
    while !input.is_empty() {
        let (input_, line) = terminated(
                take_while(|c| c != '\r' && c != '\n'),
                alt((tag("\r\n"), tag("\n"), tag(""))))
            (input)?;
        out.push(parse_line(line)?.1);
        input = input_;
    }

    Ok((input, out))
}

#[test]
pub fn test_parse_text_gemini() {
    let r = parse_text_gemini("# h1
> quote
## h2").unwrap();
    assert_eq!(r.1, vec![
        Line::H1("h1".to_string()),
        Line::Quote("quote".to_string()),
        Line::H2("h2".to_string())
    ]);
}

#[test]
pub fn test_parse_line() {
    let r = parse_line("=> hello.com world").unwrap();
    assert_eq!(r.1, Line::Link {
        url: "hello.com".to_string(),
        name: Some("world".to_string()) });

    let r = parse_line("=> hello.com ").unwrap();
    assert_eq!(r.1, Line::Link {
        url: "hello.com".to_string(),
        name: None });

    let r = parse_line("#header").unwrap();
    assert_eq!(r.1, Line::H1("header".to_string()));

    let r = parse_line("#  header").unwrap();
    assert_eq!(r.1, Line::H1("header".to_string()));

    let r = parse_line("> quote").unwrap();
    assert_eq!(r.1, Line::Quote("quote".to_string()));

    let r = parse_line("```py").unwrap();
    assert_eq!(r.1, Line::Pre { alt: Some("py".to_string()), text: String::new() });
}
