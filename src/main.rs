extern crate nom;
use nom::{
    branch::alt,
    character::streaming::{char, multispace0, none_of},
    combinator::opt,
    error::{Error, ErrorKind::Char},
    multi::{many0, many1},
    sequence::{preceded, terminated},
    Err::{self, Incomplete},
    IResult,
};
use std::{
    fmt,
    io::{self, BufRead, Write},
};

#[derive(Debug)]
enum LispData {
    Nil,
    Symbol(String),
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
            Symbol(s) => s.fmt(f),
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

fn symbol(input: &str) -> IResult<&str, LispData> {
    let (input, a) = many1(lisptoken)(input)?;
    let b: String = a.into_iter().collect();
    match b.as_str() {
        "." => Err(Err::Error(Error::new("dot", Char))),
        _ => Ok((input, Symbol(b))),
    }
}

fn lisp_data(input: &str) -> IResult<&str, LispData> {
    let (input, _) = multispace0(input)?;
    alt((cons, symbol))(input)
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
