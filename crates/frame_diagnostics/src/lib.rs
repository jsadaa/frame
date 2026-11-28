//! Compiler diagnostics, as data.
//!
//! A [`Diagnostic`] says which rule a program breaks, where, and why. The rule is named by
//! its [`RuleCode`], such as `MUT-4`. There is no severity: every diagnostic is an error
//! (DIA-2).
//!
//! [`render()`] turns a diagnostic into text for the terminal. Its documentation shows a
//! complete example.

mod render;

use frame_source::{FileId, Span};
pub use render::render;
use std::fmt;

/// The part of the specification a rule belongs to, such as `MUT` in `MUT-4`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Prefix {
    /// Conformance.
    Cnf,
    /// Lexical structure.
    Lex,
    /// Syntax.
    Syn,
    /// Names, scopes and visibility.
    Nam,
    /// Modules and imports.
    Mod,
    /// Types.
    Typ,
    /// Local inference.
    Inf,
    /// Values and places.
    Val,
    /// Mutation and exclusivity.
    Mut,
    /// Expressions.
    Exp,
    /// Statements.
    Stm,
    /// Patterns.
    Pat,
    /// Functions.
    Fun,
    /// Failures, absence and panics.
    Err,
    /// Contracts.
    Con,
    /// Interfaces.
    Ifc,
    /// Capabilities.
    Cap,
    /// Resources.
    Res,
    /// Closures.
    Clo,
    /// Collections.
    Col,
    /// Text and formatting.
    Txt,
    /// Packages and manifest.
    Pkg,
    /// Scripts.
    Scr,
    /// Tests.
    Tst,
    /// Running a program.
    Run,
    /// Standard library.
    Std,
    /// The `frame` command.
    Cli,
    /// Diagnostics.
    Dia,
    /// The formatter.
    Fmt,
    /// The reference implementation.
    Imp,
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self {
            Prefix::Cnf => "CNF",
            Prefix::Lex => "LEX",
            Prefix::Syn => "SYN",
            Prefix::Nam => "NAM",
            Prefix::Mod => "MOD",
            Prefix::Typ => "TYP",
            Prefix::Inf => "INF",
            Prefix::Val => "VAL",
            Prefix::Mut => "MUT",
            Prefix::Exp => "EXP",
            Prefix::Stm => "STM",
            Prefix::Pat => "PAT",
            Prefix::Fun => "FUN",
            Prefix::Err => "ERR",
            Prefix::Con => "CON",
            Prefix::Ifc => "IFC",
            Prefix::Cap => "CAP",
            Prefix::Res => "RES",
            Prefix::Clo => "CLO",
            Prefix::Col => "COL",
            Prefix::Txt => "TXT",
            Prefix::Pkg => "PKG",
            Prefix::Scr => "SCR",
            Prefix::Tst => "TST",
            Prefix::Run => "RUN",
            Prefix::Std => "STD",
            Prefix::Cli => "CLI",
            Prefix::Dia => "DIA",
            Prefix::Fmt => "FMT",
            Prefix::Imp => "IMP",
        };

        f.write_str(prefix)
    }
}

/// The code of a specification rule, such as `MUT-4`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RuleCode {
    prefix: Prefix,
    number: u32,
}

impl RuleCode {
    /// Creates the code `prefix-number`.
    ///
    /// This is a `const fn`, so that rule codes can be declared as constants.
    pub const fn new(prefix: Prefix, number: u32) -> Self {
        Self { prefix, number }
    }

    /// Returns the prefix.
    pub fn prefix(&self) -> Prefix {
        self.prefix
    }

    /// Returns the number.
    pub fn number(&self) -> u32 {
        self.number
    }
}

impl fmt::Display for RuleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.prefix, self.number)
    }
}

/// Where a diagnostic points: either a span, underlined with `^` when rendered, or a whole
/// file.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Primary {
    /// A whole file, for an error that no line can show, such as a file that is too large.
    File {
        /// The file.
        file: FileId,
    },
    /// A span of source text.
    Span {
        /// The offending text.
        span: Span,
        /// The message printed after the `^` markers. It must fit on one line, which
        /// [`Diagnostic::new`] checks.
        message: Option<String>,
    },
}

impl Primary {
    /// Creates a primary location for a whole file.
    pub fn file(file: FileId) -> Self {
        Self::File { file }
    }

    /// Creates a primary location at `span`, without a message.
    pub fn span(span: Span) -> Self {
        Self::Span {
            span,
            message: None,
        }
    }

    /// Creates a primary location at `span`, with `message` printed after the `^` markers.
    pub fn span_with_message(span: Span, message: impl Into<String>) -> Self {
        Self::Span {
            span,
            message: Some(message.into()),
        }
    }
}

/// A secondary label: another span that helps explain the error, with a message.
///
/// It is underlined with `-` when rendered.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Label {
    span: Span,
    message: String,
}

impl Label {
    /// Creates a label on `span` with `message`.
    ///
    /// # Panics
    ///
    /// Panics if `message` is empty or does not fit on one line.
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        let message = message.into();

        assert_text_fits_on_one_line(&message, "message of a label");
        assert_not_empty(&message, "message of a label");

        Self { span, message }
    }

    /// Returns the span.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// An automatic fix: a span and the text that replaces it.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Fix {
    span: Span,
    replacement: String,
}

impl Fix {
    /// Creates the fix that replaces `span` with `replacement`.
    pub fn new(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    /// Returns the span to replace.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns the replacement text. Unlike the other texts of a diagnostic, it may cover
    /// several lines.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// A compiler error: the violated rule, a title, where, and why.
///
/// [`Diagnostic::new`] takes what every diagnostic needs, and the `with_` methods add the
/// optional parts in the same expression. The example of [`render()`] builds one.
#[derive(Debug, PartialEq, Eq, Clone)]
// A diagnostic that is built and then dropped is an error the user never sees.
#[must_use]
pub struct Diagnostic {
    code: RuleCode,
    title: String,
    primary: Primary,
    labels: Vec<Label>,
    reason: Option<String>,
    help: Option<String>,
    fix: Option<Fix>,
}

impl Diagnostic {
    /// Creates a diagnostic for the rule `code`, with a title, at `primary`.
    ///
    /// An empty message on `primary` is allowed, and counts as no message.
    ///
    /// # Panics
    ///
    /// Panics if `title` is empty, or if `title` or the message of `primary` does not fit on
    /// one line.
    pub fn new(code: RuleCode, title: impl Into<String>, primary: Primary) -> Self {
        let title = title.into();

        assert_text_fits_on_one_line(&title, "title");
        assert_not_empty(&title, "title");

        if let Primary::Span {
            message: Some(message),
            ..
        } = &primary
        {
            assert_text_fits_on_one_line(message, "message of the primary location");
        }

        Self {
            code,
            title,
            primary,
            labels: Vec::new(),
            reason: None,
            help: None,
            fix: None,
        }
    }

    /// Adds a secondary label after the existing ones.
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Sets the reason, which explains why the rule rejects the program.
    ///
    /// # Panics
    ///
    /// Panics if `reason` is empty or does not fit on one line.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();

        assert_text_fits_on_one_line(&reason, "reason");
        assert_not_empty(&reason, "reason");

        self.reason = Some(reason);
        self
    }

    /// Sets the help, a concrete correction such as the code to write.
    ///
    /// # Panics
    ///
    /// Panics if `help` is empty or does not fit on one line.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        let help = help.into();

        assert_text_fits_on_one_line(&help, "help");
        assert_not_empty(&help, "help");

        self.help = Some(help);
        self
    }

    /// Sets the automatic fix. A diagnostic has at most one, so this replaces any previous
    /// fix.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Returns the code of the violated rule.
    pub fn code(&self) -> RuleCode {
        self.code
    }

    /// Returns the title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the primary location.
    pub fn primary(&self) -> &Primary {
        &self.primary
    }

    /// Returns the secondary labels, in the order they were added.
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// Returns the reason, if any.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Returns the help, if any.
    pub fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// Returns the automatic fix, if any.
    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }
}

/// Rendering adds the line endings itself, so `text` can't hold a `\n`, not even at the end.
fn assert_text_fits_on_one_line(text: &str, field: &str) {
    assert!(
        !text.contains('\n'),
        "the {field} of a diagnostic must fit on one line: {text:?}"
    );
}

/// An empty text says nothing, and would leave a trailing space in the rendering.
fn assert_not_empty(text: &str, field: &str) {
    assert!(
        !text.is_empty(),
        "the {field} of a diagnostic cannot be empty"
    );
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Fix, Label, Prefix, Primary, RuleCode};
    use frame_source::{FileId, SourceMap, Span};

    const INVALID_UTF8: RuleCode = RuleCode::new(Prefix::Lex, 1);
    const REFUSED_TOKEN: RuleCode = RuleCode::new(Prefix::Lex, 23);

    fn add_file(text: &str) -> FileId {
        SourceMap::new().add(String::from("demo.fm"), text.to_string())
    }

    #[test]
    fn prefixes_are_written_in_capitals() {
        let cases: [(Prefix, &str); 30] = [
            (Prefix::Cap, "CAP"),
            (Prefix::Cli, "CLI"),
            (Prefix::Clo, "CLO"),
            (Prefix::Col, "COL"),
            (Prefix::Con, "CON"),
            (Prefix::Cnf, "CNF"),
            (Prefix::Dia, "DIA"),
            (Prefix::Err, "ERR"),
            (Prefix::Exp, "EXP"),
            (Prefix::Fmt, "FMT"),
            (Prefix::Fun, "FUN"),
            (Prefix::Ifc, "IFC"),
            (Prefix::Imp, "IMP"),
            (Prefix::Inf, "INF"),
            (Prefix::Lex, "LEX"),
            (Prefix::Mod, "MOD"),
            (Prefix::Mut, "MUT"),
            (Prefix::Nam, "NAM"),
            (Prefix::Pat, "PAT"),
            (Prefix::Pkg, "PKG"),
            (Prefix::Res, "RES"),
            (Prefix::Run, "RUN"),
            (Prefix::Scr, "SCR"),
            (Prefix::Std, "STD"),
            (Prefix::Stm, "STM"),
            (Prefix::Syn, "SYN"),
            (Prefix::Tst, "TST"),
            (Prefix::Txt, "TXT"),
            (Prefix::Typ, "TYP"),
            (Prefix::Val, "VAL"),
        ];

        for (prefix, expected) in cases {
            assert_eq!(prefix.to_string(), expected, "{prefix:?}");
        }
    }

    #[test]
    fn rule_codes_are_written_prefix_dash_number() {
        let cases = [
            (RuleCode::new(Prefix::Mut, 4), "MUT-4"),
            (RuleCode::new(Prefix::Cnf, 6), "CNF-6"),
            (RuleCode::new(Prefix::Lex, 1), "LEX-1"),
            (RuleCode::new(Prefix::Lex, 23), "LEX-23"),
        ];

        for (code, expected) in cases {
            assert_eq!(code.to_string(), expected, "{code:?}");
        }
    }

    #[test]
    fn rule_code_can_be_a_constant() {
        assert_eq!(INVALID_UTF8.to_string(), "LEX-1");
        assert_eq!(INVALID_UTF8.prefix(), Prefix::Lex);
        assert_eq!(INVALID_UTF8.number(), 1);
    }

    #[test]
    fn minimal_diagnostic_has_no_optional_element() {
        let file = add_file("let \u{FFFD} = 1\n");
        let diagnostic = Diagnostic::new(
            INVALID_UTF8,
            "source file is not valid UTF-8",
            Primary::span(Span::new(file, 4, 7)),
        );

        assert_eq!(diagnostic.code(), RuleCode::new(Prefix::Lex, 1));
        assert_eq!(diagnostic.title(), "source file is not valid UTF-8");
        assert_eq!(
            diagnostic.primary(),
            &Primary::Span {
                span: Span::new(file, 4, 7),
                message: None,
            }
        );
        assert_eq!(diagnostic.labels(), &Vec::new());
        assert_eq!(diagnostic.reason(), None);
        assert_eq!(diagnostic.help(), None);
        assert_eq!(diagnostic.fix(), None);
    }

    #[test]
    fn every_added_element_is_read_back() {
        let file = add_file("if a && b {\n}\n");
        let diagnostic = Diagnostic::new(
            REFUSED_TOKEN,
            "`&&` is not an operator",
            Primary::span_with_message(Span::new(file, 5, 7), "use `and`"),
        )
        .with_label(Label::new(Span::new(file, 3, 9), "in this condition"))
        .with_reason("logical operators are written as words")
        .with_help("write `and` instead of `&&`")
        .with_fix(Fix::new(Span::new(file, 5, 7), "and"));

        assert_eq!(diagnostic.code(), RuleCode::new(Prefix::Lex, 23));
        assert_eq!(diagnostic.title(), "`&&` is not an operator");
        assert_eq!(
            diagnostic.primary(),
            &Primary::Span {
                span: Span::new(file, 5, 7),
                message: Some(String::from("use `and`")),
            }
        );
        assert_eq!(
            diagnostic.labels(),
            &[Label {
                span: Span::new(file, 3, 9),
                message: String::from("in this condition"),
            }]
        );
        assert_eq!(
            diagnostic.reason(),
            Some("logical operators are written as words")
        );
        assert_eq!(diagnostic.help(), Some("write `and` instead of `&&`"));
        assert_eq!(
            diagnostic.fix(),
            Some(&Fix {
                span: Span::new(file, 5, 7),
                replacement: String::from("and"),
            })
        );
    }

    #[test]
    fn labels_are_read_back_in_the_order_they_were_added() {
        let file = add_file("if a && b {\n}\n");
        let diagnostic = Diagnostic::new(
            REFUSED_TOKEN,
            "`&&` is not an operator",
            Primary::span(Span::new(file, 5, 7)),
        )
        .with_label(Label::new(Span::new(file, 8, 9), "added first"))
        .with_label(Label::new(Span::new(file, 3, 4), "added second"))
        .with_label(Label::new(Span::new(file, 0, 2), "added third"));

        assert_eq!(
            diagnostic.labels(),
            &[
                Label {
                    span: Span::new(file, 8, 9),
                    message: String::from("added first"),
                },
                Label {
                    span: Span::new(file, 3, 4),
                    message: String::from("added second"),
                },
                Label {
                    span: Span::new(file, 0, 2),
                    message: String::from("added third"),
                },
            ]
        );
    }

    #[test]
    fn label_is_read_back_through_its_accessors() {
        let file = add_file("if a && b {\n}\n");
        let label = Label::new(Span::new(file, 3, 9), "in this condition");

        assert_eq!(label.span(), Span::new(file, 3, 9));
        assert_eq!(label.message(), "in this condition");
    }

    #[test]
    fn primary_location_can_be_a_whole_file() {
        let file = add_file("");
        let diagnostic = Diagnostic::new(
            RuleCode::new(Prefix::Cnf, 6),
            "source file is too large",
            Primary::file(file),
        );

        assert_eq!(diagnostic.primary(), &Primary::File { file });
    }

    #[test]
    fn fix_replacement_may_cover_several_lines() {
        let span = Span::new(add_file("a; b\n"), 1, 3);
        let fix = Fix::new(span, "\n");

        assert_eq!(fix.span(), span);
        assert_eq!(fix.replacement(), "\n");
    }

    #[test]
    #[should_panic(expected = "the title of a diagnostic must fit on one line: \"two\\nlines\"")]
    fn rejects_a_title_on_two_lines() {
        let _ = Diagnostic::new(INVALID_UTF8, "two\nlines", Primary::file(add_file("")));
    }

    #[test]
    #[should_panic(expected = "the title of a diagnostic must fit on one line: \"title\\n\"")]
    fn rejects_a_title_ending_with_a_newline() {
        let _ = Diagnostic::new(INVALID_UTF8, "title\n", Primary::file(add_file("")));
    }

    #[test]
    #[should_panic(
        expected = "the message of the primary location of a diagnostic must fit on one line: \"two\\nlines\""
    )]
    fn rejects_a_primary_message_on_two_lines() {
        let _ = Diagnostic::new(
            RuleCode::new(Prefix::Cnf, 6),
            "title",
            Primary::Span {
                span: Span::new(add_file("if a && b {\n}\n"), 5, 7),
                message: Some(String::from("two\nlines")),
            },
        );
    }

    #[test]
    #[should_panic(
        expected = "the message of a label of a diagnostic must fit on one line: \"two\\nlines\""
    )]
    fn rejects_a_label_on_two_lines() {
        let span = Span::new(add_file("if a && b {\n}\n"), 3, 4);
        Label::new(span, "two\nlines");
    }

    #[test]
    #[should_panic(expected = "the reason of a diagnostic must fit on one line: \"two\\nlines\"")]
    fn rejects_a_reason_on_two_lines() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_reason("two\nlines");
    }

    #[test]
    #[should_panic(expected = "the help of a diagnostic must fit on one line: \"two\\nlines\"")]
    fn rejects_a_help_on_two_lines() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_help("two\nlines");
    }

    #[test]
    #[should_panic(expected = "the title of a diagnostic cannot be empty")]
    fn rejects_an_empty_title() {
        let _ = Diagnostic::new(INVALID_UTF8, "", Primary::file(add_file("")));
    }

    #[test]
    #[should_panic(expected = "the message of a label of a diagnostic cannot be empty")]
    fn rejects_an_empty_label() {
        let span = Span::new(add_file("if a && b {\n}\n"), 3, 4);
        Label::new(span, "");
    }

    #[test]
    #[should_panic(expected = "the reason of a diagnostic cannot be empty")]
    fn rejects_an_empty_reason() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_reason("");
    }

    #[test]
    #[should_panic(expected = "the help of a diagnostic cannot be empty")]
    fn rejects_an_empty_help() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_help("");
    }

    #[test]
    fn primary_message_may_be_empty() {
        let span = Span::new(add_file("if a && b {\n}\n"), 5, 7);
        let diagnostic = Diagnostic::new(
            REFUSED_TOKEN,
            "`&&` is not an operator",
            Primary::span_with_message(span, ""),
        );

        assert_eq!(
            diagnostic.primary(),
            &Primary::Span {
                span,
                message: Some(String::new()),
            }
        );
    }
}
