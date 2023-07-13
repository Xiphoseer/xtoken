#![no_std]
//! # xtoken
//!
//! [`Iterator`] based `no_std` XML Tokenizer using [`memchr`](https://docs.rs/memchr).
//!
//! ## Design Goals
//!
//! - Operates on byte slices
//! - Minimal Validation
//! - Support for inline DTD declaration
//! - Partition whole input into non-empty spans
//!
//! ## Example
//!
//! ```
//! use xtoken::{Token, Tokenizer};
//!
//! let tokens = Tokenizer::new(b"<x>Hello World!</x>").collect::<Vec<_>>();
//! assert_eq!(&tokens, &[
//!     Token::Element(b"<x>"),
//!     Token::Span(b"Hello World!"),
//!     Token::ElementEnd(b"</x>"),
//! ]);
//!
//! let tokens = Tokenizer::new(b"<!DOCTYPE xml>").collect::<Vec<_>>();
//! assert_eq!(&tokens, &[
//!     Token::Decl(b"<!DOCTYPE xml>")
//! ]);
//! ```

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    /// Non-Syntax
    Span(&'a [u8]),
    /// Entity (i.e. `&...;`)
    Entity(&'a [u8]),
    /// Malformed Tokens
    Error(&'a [u8]),
    /// Processing Instruction (i.e. `<? ... ?>`)
    PI(&'a [u8]),
    /// Comment (i.e. `<!-- ... -->`)
    Comment(&'a [u8]),
    /// Structural Declaration, e.g. `<!DOCTYPE ... >`
    Decl(&'a [u8]),
    /// End of `Decl` with body (e.g. `]>`)
    DeclEnd(&'a [u8]),
    /// Element
    Element(&'a [u8]),
    /// End of Element (i.e. `</...>`)
    ElementEnd(&'a [u8]),
}

pub struct Tokenizer<'a> {
    rest: &'a [u8],
    depth: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            rest: bytes,
            depth: 0,
        }
    }

    fn rest_err(&mut self) -> Token<'a> {
        let (span, rest) = self.rest.split_at(self.rest.len());
        self.rest = rest;
        Token::Error(span)
    }

    fn proc(&mut self, mut rest: &'a [u8]) -> Token<'a> {
        loop {
            if let Some(pos) = memchr::memchr(b'?', rest) {
                rest = &rest[(pos + 1)..];
                if let Some((&chr2, rest2)) = rest.split_first() {
                    if chr2 == b'>' {
                        let span = &self.rest[..(self.rest.len() - rest2.len())];
                        self.rest = rest2;
                        break Token::PI(span);
                    }
                } else {
                    break self.rest_err();
                }
            } else {
                break self.rest_err();
            }
        }
    }

    fn comment(&mut self, mut rest: &'a [u8]) -> Token<'a> {
        loop {
            if let Some(pos) = memchr::memchr(b'-', rest) {
                rest = &rest[(pos + 1)..];
                if let Some((&chr2, rest2)) = rest.split_first() {
                    if chr2 == b'-' {
                        if rest2.starts_with(b">") {
                            let mid = self.rest.len() - (rest2.len() - 1);
                            let (span, rest) = self.rest.split_at(mid);
                            self.rest = rest;
                            break Token::Comment(span);
                        } else {
                            // technically invalid, but ignore
                        }
                    }
                } else {
                    break self.rest_err();
                }
            } else {
                break self.rest_err();
            }
        }
    }

    fn decl(&mut self, rest: &'a [u8]) -> Token<'a> {
        if let Some(pos) = memchr::memchr2(b'>', b'[', rest) {
            let mid = self.rest.len() - (rest.len() - (pos + 1));
            let (span, rest) = self.rest.split_at(mid);
            self.rest = rest;
            if span[mid - 1] == b'[' {
                self.depth += 1;
            }
            Token::Decl(span)
        } else {
            self.rest_err()
        }
    }

    fn decl_end(&mut self) -> Token<'a> {
        if let Some(pos) = memchr::memchr(b'>', self.rest) {
            let (span, rest) = self.rest.split_at(pos + 1);
            self.rest = rest;
            Token::DeclEnd(span)
        } else {
            self.rest_err()
        }
    }

    fn builtin(&mut self, rest: &'a [u8]) -> Token<'a> {
        if rest.starts_with(b"--") {
            self.comment(&rest[2..])
        } else {
            match rest.first().copied() {
                Some(b'A'..=b'Z') => self.decl(rest),
                None => self.rest_err(),
                _ => todo!(),
            }
        }
    }

    fn entity(&mut self) -> Token<'a> {
        // entity
        if let Some(pos) = memchr::memchr(b';', self.rest) {
            let (span, rest) = self.rest.split_at(pos + 1);
            self.rest = rest;
            Token::Entity(span)
        } else {
            self.rest_err()
        }
    }

    fn element(&mut self) -> Token<'a> {
        if let Some(pos) = memchr::memchr(b'>', self.rest) {
            let (span, rest) = self.rest.split_at(pos + 1);
            self.rest = rest;
            Token::Element(span)
        } else {
            self.rest_err()
        }
    }

    fn element_end(&mut self) -> Token<'a> {
        if let Some(pos) = memchr::memchr(b'>', self.rest) {
            let (span, rest) = self.rest.split_at(pos + 1);
            self.rest = rest;
            Token::ElementEnd(span)
        } else {
            self.rest_err()
        }
    }

    fn structure(&mut self) -> Token<'a> {
        let inner = &self.rest[1..];
        if let Some((&chr, rest)) = inner.split_first() {
            match chr {
                b'!' => self.builtin(rest),
                b'?' => self.proc(rest),
                b'/' => self.element_end(),
                _ => self.element(),
            }
        } else {
            self.rest_err()
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pos) = match self.depth {
            0 => memchr::memchr2(b'<', b'&', self.rest),
            _ => memchr::memchr3(b'<', b'&', b']', self.rest),
        } {
            if pos > 0 {
                let (span, rest) = self.rest.split_at(pos);
                self.rest = rest;
                Some(Token::Span(span))
            } else {
                let first = self.rest[pos];
                match first {
                    b'&' => Some(self.entity()),
                    b'<' => Some(self.structure()),
                    b']' => Some(self.decl_end()),
                    _ => unreachable!(),
                }
            }
        } else {
            let len = self.rest.len();
            if len > 0 {
                let (span, rest) = self.rest.split_at(len);
                self.rest = rest;
                Some(Token::Span(span))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Token, Tokenizer};

    const XML_SCHEMA: &str = include_str!("../../XMLSchema.xsd");

    #[test]
    fn test_tokens() {
        let mut t = Tokenizer::new(XML_SCHEMA.as_bytes());
        assert_eq!(
            t.next().unwrap(),
            Token::PI(b"<?xml version='1.0' encoding='UTF-8'?>")
        );
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Comment(b"<!-- XML Schema schema for XML Schemas: Part 1: Structures -->")
        );
        for _ in 0..4 {
            assert!(matches!(t.next(), Some(Token::Span(_))));
            assert!(matches!(t.next(), Some(Token::Comment(_))));
        }
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!DOCTYPE xs:schema PUBLIC \"-//W3C//DTD XMLSCHEMA 200102//EN\" \"XMLSchema.dtd\" [")
        );
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert!(matches!(t.next(), Some(Token::Comment(_))));
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!ATTLIST xs:schema          id  ID  #IMPLIED>")
        );
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!ATTLIST xs:complexType     id  ID  #IMPLIED>")
        );
        for _ in 0..21 {
            assert!(matches!(t.next(), Some(Token::Span(_))));
            assert!(matches!(t.next(), Some(Token::Decl(_))));
        }
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert!(matches!(t.next(), Some(Token::Comment(_))));

        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!ENTITY % schemaAttrs 'xmlns:hfp CDATA #IMPLIED'>")
        );
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!ELEMENT hfp:hasFacet EMPTY>")
        );
        for _ in 0..3 {
            assert!(matches!(t.next(), Some(Token::Span(_))));
            assert!(matches!(t.next(), Some(Token::Decl(_))));
        }

        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert!(matches!(t.next(), Some(Token::Comment(_))));

        for _ in 0..15 {
            assert!(matches!(t.next(), Some(Token::Span(_))));
            assert!(matches!(t.next(), Some(Token::Decl(_))));
        }

        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(
            t.next().unwrap(),
            Token::Decl(b"<!ATTLIST xs:union id ID #IMPLIED>")
        );
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(t.next().unwrap(), Token::DeclEnd(b"]>"));

        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(t.next().unwrap(), Token::Element(br##"<xs:schema targetNamespace="http://www.w3.org/2001/XMLSchema" blockDefault="#all" elementFormDefault="qualified" version="1.0" xmlns:xs="http://www.w3.org/2001/XMLSchema" xml:lang="EN" xmlns:hfp="http://www.w3.org/2001/XMLSchema-hasFacetAndProperty">"##));
        assert!(matches!(t.next(), Some(Token::Span(_))));
        assert_eq!(t.next().unwrap(), Token::Element(br##"<xs:annotation>"##));

        let mut count = 0;
        while let Some(_token) = t.next() {
            count += 1;
        }
        assert_eq!(count, 4188);
    }
}
