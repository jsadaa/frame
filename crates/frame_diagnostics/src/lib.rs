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
    ///
    /// # Panics
    ///
    /// Panics if `message` does not fit on one line.
    pub fn span(span: Span, message: Option<String>) -> Self {
        if let Some(message) = message.clone() {
            assert!(
                text_fits_on_one_line(&message),
                "expected one line of text, got: {message}"
            );
        }

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
    /// Panics if `title` does not fit on one line.
    pub fn new(code: RuleCode, title: impl Into<String>, primary: Primary) -> Self {
        let title = title.into();

        assert!(
            text_fits_on_one_line(&title),
            "expected one line of text, got: {title}",
        );

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
    if let Some(position) = text.find("\n")
        && text.get(position + 1..).is_some()
    {
        return true;
    }

    if text.matches("\n").count() <= 1 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {}
