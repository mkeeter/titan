use std::convert::TryFrom;

use nom::{
  IResult,
  bytes::complete::{tag, take_while_m_n, is_not},
  character::complete::char,
  combinator::map_res,
  sequence::delimited};

use crate::protocol::{ResponseStatus, ResponseHeader};
use anyhow::Result;

fn parse_status(i: &[u8]) -> Result<ResponseStatus> {
    let s = std::str::from_utf8(i)?;
    ResponseStatus::try_from(u32::from_str_radix(s, 10)?)
}

pub fn parse_header(input: &[u8]) -> IResult<&[u8], ResponseHeader> {
    let (input, status) = map_res(
        take_while_m_n(2, 2, |c: u8| (c as char).is_digit(10)),
        parse_status
    )(input)?;

    let (input, _) = tag(" ")(input)?;
    let (input, meta) = map_res(
        take_while_m_n(0, 1024, |c: u8| c as char != '\r'),
        std::str::from_utf8
    )(input)?;
    let (input, _) = tag("\r\n")(input)?;

    let meta = meta.to_string();

    Ok((input, ResponseHeader { status, meta }))
}

#[test]
pub fn test_parse_header() {
    let s = "20 text/gemini\r\nomg wtf bbq lol";
    parse_header(s.as_bytes());
}
