use std::convert::TryFrom;

use anyhow::Result;

use nom::{
  IResult,
  bytes::complete::{tag, take_while_m_n, take_until, take_while},
  character::{is_space, is_digit},
  combinator::map_res};

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
    Ok((input, Line::List(input.to_string())))
}

pub fn parse_line_quote(input: &str) -> IResult<&str, Line> {
    let (input, _) = tag(">")(input)?;
    Ok((input, Line::List(input.to_string())))
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
