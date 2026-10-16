//! Diagnostics: how the compiler reports a refused program, as data (DIA-1).
//!
//! A [`Diagnostic`] names the violated rule by its [`RuleCode`], such as `MUT-4`, and says
//! where and why the program is refused. It has no severity: every diagnostic is an error
//! (DIA-2).

use frame_source::{FileId, Span};
use std::fmt::Display;

/// The prefix of a rule code, which names the area of the specification the rule belongs
/// to; written in capitals, such as `MUT`.
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

impl Display for Prefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

        write!(f, "{prefix}")
    }
}

/// The code of a rule of the specification, such as `MUT-4`: a prefix and a number.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RuleCode {
    prefix: Prefix,
    number: u32,
}

impl RuleCode {
    /// Creates the code `prefix-number`; a `const fn`, so that a rule's code can be a
    /// constant.
    pub const fn new(prefix: Prefix, number: u32) -> Self {
        Self { prefix, number }
    }

    /// Returns the prefix.
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    /// Returns the number.
    pub fn number(&self) -> u32 {
        self.number
    }
}

impl Display for RuleCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.prefix, self.number)
    }
}

/// The primary location of a diagnostic: the span in cause, which the rendering underlines
/// with `^`, or a whole file.
#[derive(Debug, PartialEq, Eq)]
pub enum Primary {
    /// A whole file, for an error that no line shows, such as a file too large (CNF-6).
    File {
        /// The file.
        file: FileId,
    },
    /// A span of the text.
    Span {
        /// The text in cause.
        span: Span,
        /// The message written after the `^`, on one line.
        message: Option<String>,
    },
}

impl Primary {
    /// Creates the primary location of a whole file.
    pub fn file(file: FileId) -> Self {
        Self::File { file }
    }

    /// Creates the primary location of `span`, with an optional message.
    pub fn span(span: Span, message: Option<String>) -> Self {
        Self::Span { span, message }
    }
}

/// A secondary label: another span that explains the error, which the rendering underlines
/// with `-`, and its message.
#[derive(Debug, PartialEq, Eq)]
pub struct Label {
    span: Span,
    message: String,
}

impl Label {
    /// Creates the label of `span`.
    ///
    /// # Panics
    ///
    /// Panics if `message` does not fit on one line.
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        let message = message.into();

        assert!(
            text_fits_on_one_line(&message),
            "expected one line of text, got: {message}"
        );

        Self { span, message }
    }
}

/// An automatic fix: the text that replaces a span (DIA-1).
#[derive(Debug, PartialEq, Eq)]
pub struct Fix {
    span: Span,
    replacement: String,
}

impl Fix {
    /// Creates the fix that replaces `span` with `replacement`.
    pub fn new(span: Span, replacement: String) -> Self {
        Self { span, replacement }
    }

    /// Returns the span to replace.
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the text that replaces the span; unlike the other texts of a diagnostic, it
    /// may cover several lines.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// An error of the compiler: the violated rule, a title, where, and why (DIA-1).
///
/// [`Diagnostic::new`] takes what every diagnostic has; the `with_` methods add the rest, in
/// the same expression.
#[derive(Debug, PartialEq, Eq)]
// Built, then dropped, a diagnostic is an error that the user never sees.
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
    /// Creates a diagnostic of the rule `code`, titled `title`, at `primary`.
    ///
    /// # Panics
    ///
    /// Panics if `title`, or `primary` message does not fit on one line.
    pub fn new(code: RuleCode, title: impl Into<String>, primary: Primary) -> Self {
        let title = title.into();

        assert!(
            text_fits_on_one_line(&title),
            "expected one line of text, got: {title}",
        );

        if let Primary::Span {
            message: Some(ref text),
            ..
        } = primary
        {
            assert!(
                text_fits_on_one_line(text),
                "expected one line of text, got: {text}"
            );
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

    /// Adds a secondary label, after those already added.
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Sets the reason: why the rule refuses the program.
    ///
    /// # Panics
    ///
    /// Panics if `reason` does not fit on one line.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();

        assert!(
            text_fits_on_one_line(&reason),
            "expected one line of text, got: {reason}"
        );

        self.reason = Some(reason);
        self
    }

    /// Sets the help: a concrete correction, such as the text to write.
    ///
    /// # Panics
    ///
    /// Panics if `help` does not fit on one line.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        let help = help.into();

        assert!(
            text_fits_on_one_line(&help),
            "expected one line of text, got: {help}"
        );

        self.help = Some(help);
        self
    }

    /// Sets the automatic fix; a diagnostic has one at most.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Returns the code of the violated rule.
    pub fn code(&self) -> &RuleCode {
        &self.code
    }

    /// Returns the title.
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// Returns the primary location.
    pub fn primary(&self) -> &Primary {
        &self.primary
    }

    /// Returns the secondary labels, in the order they were added.
    pub fn labels(&self) -> &Vec<Label> {
        &self.labels
    }

    /// Returns the reason, if any.
    pub fn reason(&self) -> Option<&String> {
        self.reason.as_ref()
    }

    /// Returns the help, if any.
    pub fn help(&self) -> Option<&String> {
        self.help.as_ref()
    }

    /// Returns the automatic fix, if any.
    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }
}

/// Returns `true` if `text` fits on one line: at most one `\n`, at its end.
fn text_fits_on_one_line(text: &str) -> bool {
    !text.contains("\n")
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
        assert_eq!(INVALID_UTF8.prefix(), &Prefix::Lex);
        assert_eq!(INVALID_UTF8.number(), 1);
    }

    #[test]
    fn minimal_diagnostic_has_no_optional_element() {
        let file = add_file("let \u{FFFD} = 1\n");
        let diagnostic = Diagnostic::new(
            INVALID_UTF8,
            "source file is not valid UTF-8",
            Primary::span(Span::new(file, 4, 7), None),
        );

        assert_eq!(diagnostic.code(), &RuleCode::new(Prefix::Lex, 1));
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
            Primary::span(Span::new(file, 5, 7), Some(String::from("use `and`"))),
        )
        .with_label(Label::new(Span::new(file, 3, 9), "in this condition"))
        .with_reason("logical operators are written as words")
        .with_help("write `and` instead of `&&`")
        .with_fix(Fix::new(Span::new(file, 5, 7), String::from("and")));

        assert_eq!(diagnostic.code(), &RuleCode::new(Prefix::Lex, 23));
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
            Some(&String::from("logical operators are written as words"))
        );
        assert_eq!(
            diagnostic.help(),
            Some(&String::from("write `and` instead of `&&`"))
        );
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
            Primary::span(Span::new(file, 5, 7), None),
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
        let fix = Fix::new(span, String::from("\n"));

        assert_eq!(fix.span(), &span);
        assert_eq!(fix.replacement(), "\n");
    }

    #[test]
    #[should_panic(expected = "expected one line of text, got: two\nlines")]
    fn refuses_a_title_on_two_lines() {
        let _ = Diagnostic::new(INVALID_UTF8, "two\nlines", Primary::file(add_file("")));
    }

    #[test]
    #[should_panic(expected = "expected one line of text, got: two\nlines")]
    fn refuses_a_primary_message_on_two_lines() {
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
    #[should_panic(expected = "expected one line of text, got: two\nlines")]
    fn refuses_a_label_on_two_lines() {
        let span = Span::new(add_file("if a && b {\n}\n"), 3, 4);
        Label::new(span, "two\nlines");
    }

    #[test]
    #[should_panic(expected = "expected one line of text, got: two\nlines")]
    fn refuses_a_reason_on_two_lines() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_reason("two\nlines");
    }

    #[test]
    #[should_panic(expected = "expected one line of text, got: two\nlines")]
    fn refuses_a_help_on_two_lines() {
        let diagnostic = Diagnostic::new(INVALID_UTF8, "title", Primary::file(add_file("")));
        let _ = diagnostic.with_help("two\nlines");
    }
}
