extern crate nom;
use nom::{
    branch::alt,
    bytes::streaming::{tag, take_until, take_while1},
    character::streaming::{
        anychar, char, digit1, hex_digit1, line_ending, multispace0, none_of, oct_digit1,
    },
    combinator::opt,
    error::{Error, ErrorKind::Char},
    multi::{many0, many1},
    sequence::{delimited, preceded, terminated},
    Err::{self, Incomplete},
    IResult,
};
use std::{
    fmt,
    io::{self, BufRead, Write},
    ops::Neg,
};

#[derive(Debug)]
enum LispData {
    Nil,
    Bool(bool),
    Num(i64), // TODO: Big ints, floats, etc.
    Symbol(String),
    LispString(String),
    Vector(Vec<LispData>),
    Cons(Box<LispData>, Box<LispData>),
}
use LispData::*;

fn write_cdr(data: &LispData, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match data {
        Nil => Ok(()),
        Cons(a, b) => {
            write!(f, " {}", a)?;
            write_cdr(b, f)
        }
        _ => write!(f, " . {}", data),
    }
}

impl fmt::Display for LispData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Nil => write!(f, "()"),
            Bool(b) => write!(f, "#{}", if *b { 't' } else { 'f' }),
            Num(n) => n.fmt(f),
            Symbol(s) => s.fmt(f),
            LispString(s) => write!(f, "{:?}", s),
            Vector(v) => {
                write!(f, "#(")?;
                if let Some(val) = v.get(0) {
                    val.fmt(f)?;
                }
                for val in v.iter().skip(1) {
                    write!(f, " {}", val)?;
                }
                write!(f, ")")
            }
            Cons(a, b) => {
                write!(f, "({}", a)?;
                write_cdr(&b, f)?;
                write!(f, ")")
            }
        }
    }
}

fn lisptoken(input: &str) -> IResult<&str, char> {
    none_of("'()# \"\r\n")(input)
}

fn cons(input: &str) -> IResult<&str, LispData> {
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, middle) = many0(terminated(lisp_data, multispace0))(input)?;
    let (input, dot) = opt(preceded(
        terminated(char('.'), multispace0),
        terminated(lisp_data, multispace0),
    ))(input)?;
    let (input, _) = char(')')(input)?;
    Ok((
        input,
        middle
            .into_iter()
            .rev()
            .fold(dot.unwrap_or(Nil), |a, b| Cons(Box::new(b), Box::new(a))),
    ))
}

fn quote(input: &str) -> IResult<&str, LispData> {
    let (input, _) = char('\'')(input)?;
    let (input, data) = lisp_data(input)?;
    Ok((
        input,
        Cons(
            Box::new(Symbol("quote".to_string())),
            Box::new(Cons(Box::new(data), Box::new(Nil))),
        ),
    ))
}

fn symbol(input: &str) -> IResult<&str, LispData> {
    let (input, a) = many1(lisptoken)(input)?;
    let b: String = a.into_iter().collect();
    match b.as_str() {
        "." => Err(Err::Error(Error::new("dot", Char))),
        _ => Ok((input, Symbol(b))),
    }
}

fn parse_num<'a>(
    f: fn(&'a str) -> IResult<&'a str, &'a str>,
    input: &'a str,
    radix: u32,
) -> IResult<&'a str, LispData> {
    let (input, negate) = opt(char('-'))(input)?;
    let (input, data) = f(input)?;
    match i64::from_str_radix(data, radix) {
        Ok(n) => Ok((input, Num(if negate.is_some() { n.neg() } else { n }))),
        Err(_) => Err(Err::Error(Error::new("parseint", Char))),
    }
}

fn num(input: &str) -> IResult<&str, LispData> {
    parse_num(digit1, input, 10)
}

fn bin_digit1(input: &str) -> IResult<&str, &str> {
    take_while1(|a| a == '0' || a == '1')(input)
}

fn sharp(input: &str) -> IResult<&str, LispData> {
    let (input, _) = char('#')(input)?;
    let (input, c) = anychar(input)?;
    match c.to_lowercase().next().unwrap() {
        'f' => Ok((input, Bool(false))),
        't' => Ok((input, Bool(true))),
        'b' => parse_num(bin_digit1, input, 2),
        'o' => parse_num(oct_digit1, input, 8),
        'd' => num(input),
        'x' => parse_num(hex_digit1, input, 16),
        '(' => {
            let (input, _) = multispace0(input)?;
            let (input, vals) = many0(terminated(lisp_data, multispace0))(input)?;
            let (input, _) = char(')')(input)?;
            Ok((input, Vector(vals)))
        }
        _ => Err(Err::Error(Error::new("#", Char))),
    }
}

fn block_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = delimited(tag("#|"), take_until("|#"), tag("|#"))(input)?;
    Ok((input, ()))
}

fn line_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = delimited(char(';'), take_until("\n"), line_ending)(input)?;
    Ok((input, ()))
}

fn datum_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = preceded(tag("#;"), lisp_data)(input)?;
    Ok((input, ()))
}

fn comment(input: &str) -> IResult<&str, LispData> {
    let (input, _) = alt((block_comment, line_comment, datum_comment))(input)?;
    lisp_data(input)
}

fn escaped_char(input: &str) -> IResult<&str, char> {
    let (input, _) = char('\\')(input)?;
    let (input, seq) = anychar(input)?;
    let c = match seq {
        'a' => '\x07',
        'b' => '\x08',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '"' => '\"',
        '\\' => '\\',
        '|' => '|',
        _ => return Err(Err::Error(Error::new("escape", Char))),
    };
    Ok((input, c))
}

fn string(input: &str) -> IResult<&str, LispData> {
    let (input, data) = delimited(
        char('"'),
        many0(alt((none_of("\\\""), escaped_char))),
        char('"'),
    )(input)?;
    Ok((input, LispString(data.into_iter().collect())))
}

fn lisp_data(input: &str) -> IResult<&str, LispData> {
    let (input, _) = multispace0(input)?;
    alt((quote, cons, comment, sharp, num, string, symbol))(input)
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut buffer = String::new();
    let mut stdout = io::stdout();

    loop {
        if buffer.is_empty() {
            print!("> ");
            stdout.flush()?;
        }
        match lisp_data(&buffer) {
            Ok((rest, val)) => {
                buffer = rest.to_string();
                println!("{}", val);
                print!("> ");
                stdout.flush()?;
            }
            Err(Incomplete(_)) => {
                if stdin.read_line(&mut buffer)? == 0 {
                    println!("");
                    return Ok(());
                }
            }
            err => {
                println!("{:?}", err);
                buffer.clear()
            }
        }
    }
}
