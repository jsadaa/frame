//! The frame diagnostics module

use frame_source::{FileId, Span};
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// The prefix of a rule
pub enum Prefix {
    /// CNF: todo
    Cnf,
    /// LEX: todo
    Lex,
    /// SYN: todo
    Syn,
    /// NAME: todo
    Nam,
    /// MOD: todo
    Mod,
    /// TYP: todo
    Typ,
    /// INF: todo
    Inf,
    /// VAL: todo
    Val,
    /// MUT: todo
    Mut,
    /// EXP: todo
    Exp,
    /// STM: todo
    Stm,
    /// PAT: todo
    Pat,
    /// FUN: todo
    Fun,
    /// ERR: todo
    Err,
    /// CON: todo
    Con,
    /// IFC: todo
    Ifc,
    /// CAP: todo
    Cap,
    /// RES: todo
    Res,
    /// CLO: todo
    Clo,
    /// COL: todo
    Col,
    /// TXT: todo
    Txt,
    /// PKG: todo
    Pkg,
    /// SCR: todo
    Scr,
    /// TST: todo
    Tst,
    /// RUN: todo
    Run,
    /// STD: todo
    Std,
    /// CLI: todo
    Cli,
    /// DIA: todo
    Dia,
    /// FMT: todo
    Fmt,
    /// IMP: todo
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// A rule code of frame, a prefix and a number
pub struct RuleCode {
    prefix: Prefix,
    number: u32,
}

impl RuleCode {
    /// Creates a new rule code
    pub fn new(prefix: Prefix, number: u32) -> Self {
        Self { prefix, number }
    }

    /// Get the prefix of the rule
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    /// Get the number of the rule
    pub fn number(&self) -> u32 {
        self.number
    }
}

impl Display for RuleCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.prefix, self.number)
    }
}

#[derive(Debug, PartialEq, Eq)]
/// The primary location of a diagnostic, a whole file or a chunk of text in the source
pub enum Primary {
    /// The diagnostics concerns the whole file
    File {
        /// The file id
        source: FileId,
    },
    /// The diagnostic concerns a chunk of the file
    Chunk {
        /// The span of the chunk in the source
        span: Span,
        /// The wording of this location
        wording: Option<String>,
    },
}

impl Primary {
    /// Create a primary location concerning a whole file
    pub fn file(source: FileId) -> Self {
        Self::File { source }
    }

    /// Create a primary location concerning a chunk of the source
    ///
    /// # Panics
    ///
    /// Panics if the wording does not fit on one line
    pub fn chunk(span: Span, wording: Option<String>) -> Self {
        if let Some(wording) = wording.clone() {
            assert!(
                text_fits_on_one_line(&wording),
                "expected one line of text, got: {wording}"
            );
        }

        Self::Chunk { span, wording }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// A secondary location for the diagnostic
pub struct Label {
    span: Span,
    wording: String,
}

impl Label {
    /// Create a new label
    ///
    /// # Panics
    ///
    /// Panics if the wording does not fit on one line
    pub fn new(span: Span, wording: impl Into<String>) -> Self {
        let wording = wording.into();

        assert!(
            text_fits_on_one_line(&wording),
            "expected one line of text, got: {wording}"
        );

        Self { span, wording }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// The fix as proposed by the compiler
pub struct Fix {
    span: Span,
    replacement: String,
}

impl Fix {
    /// Create a new fix
    pub fn new(span: Span, replacement: String) -> Self {
        Self { span, replacement }
    }

    /// the span of the fix
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// The text that replace the span
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Debug, PartialEq, Eq)]
#[must_use]
/// A diagnostics issued by the compiler
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
    /// Create a new diagnostic from a rule code and a location
    ///
    /// # Panics
    ///
    /// Panics if the title does not fit on one line
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

    /// Add a secondary label to the diagnostic
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Add a reason to the diagnostic
    ///
    /// # Panics
    ///
    /// Panics if the reason does not fit on one line
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();

        assert!(
            text_fits_on_one_line(&reason),
            "expected one line of text, got: {reason}"
        );

        self.reason = Some(reason);
        self
    }

    /// Add an help to the diagnostic
    ///
    /// # Panics
    ///
    /// Panics if the help does not fit on one line
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        let help = help.into();

        assert!(
            text_fits_on_one_line(&help),
            "expected one line of text, got: {help}"
        );

        self.help = Some(help);
        self
    }

    /// Add a fix to the diagnostic
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// The rule code of the diagnostic
    pub fn code(&self) -> &RuleCode {
        &self.code
    }

    /// The title of the diagnostic
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// The pimary location of the diagnostic
    pub fn primary(&self) -> &Primary {
        &self.primary
    }

    /// The secondary labels of the diagnostic
    pub fn labels(&self) -> &Vec<Label> {
        &self.labels
    }

    /// The reason of the diagnostic
    pub fn reason(&self) -> Option<&String> {
        self.reason.as_ref()
    }

    /// The help pf the diagnostic
    pub fn help(&self) -> Option<&String> {
        self.help.as_ref()
    }

    /// The fix of the diagnostic
    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }
}

/// Check if the text fits on one line : only one `\n` or less, and nothing after if present
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
