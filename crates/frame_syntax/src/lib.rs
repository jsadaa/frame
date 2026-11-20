//! Lexical analysis: splitting a file's text into tokens.
//!
//! [`tokenize`] reads a file and returns its tokens, with a diagnostic for each error it
//! finds. [`dump_tokens`] writes tokens as text, one per line.
//!
//! # Examples
//!
//! ```
//! use frame_source::SourceMap;
//! use frame_syntax::{dump_tokens, tokenize};
//!
//! let mut source_map = SourceMap::new();
//! let file = source_map.add(String::from("demo.fm"), String::from("+ <<=\n"));
//! let lexed = tokenize(&source_map, file);
//!
//! let dump = dump_tokens(lexed.tokens(), source_map.file(file));
//! assert_eq!(
//!     dump.lines().collect::<Vec<_>>(),
//!     [
//!         r#"1:1 Plus "+""#,
//!         r#"1:3 ShlEq "<<=""#,
//!         r#"1:6 Newline "\n""#,
//!         r#"2:1 Eof """#,
//!     ]
//! );
//! assert!(lexed.diagnostics().is_empty());
//! ```

mod lexer;

pub use lexer::{Lexed, Token, TokenKind, dump_tokens, tokenize};
