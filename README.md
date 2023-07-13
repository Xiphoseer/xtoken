# xtoken

`Iterator` based `no_std` XML Tokenizer using [`memchr`](https://docs.rs/memchr).

## Design Goals

- Operates on byte slices
- Minimal Validation
- Support for inline DTD declaration
- Partition whole input into non-empty spans

## Example

```rust
use xtoken::{Token, Tokenizer};

let tokens = Tokenizer::new(b"<x>Hello World!</x>").collect::<Vec<_>>();
assert_eq!(&tokens, &[
    Token::Element(b"<x>"),
    Token::Span(b"Hello World!"),
    Token::ElementEnd(b"</x>"),
]);

let tokens = Tokenizer::new(b"<!DOCTYPE xml>").collect::<Vec<_>>();
assert_eq!(&tokens, &[
    Token::Decl(b"<!DOCTYPE xml>")
]);
```
