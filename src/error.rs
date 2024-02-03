use crate::Mark;

#[derive(Debug, thiserror::Error)]
pub enum EmitterError {
    #[error("{0}")]
    Problem(&'static str),
    #[error(transparent)]
    Writer(#[from] WriterError),
}

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ReaderError {
    #[error("{problem}")]
    Problem {
        problem: &'static str,
        offset: usize,
        value: i32,
    },
    #[error("input stream produced an invalid byte order marker")]
    InvalidBom,
    #[error("invalid UTF-8 byte at offset: {value:x}")]
    InvalidUtf8 { value: u8 },
    #[error("invalid UTF-16 unpaired surrogate: {value:x}")]
    InvalidUtf16 { value: u16 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ScannerError {
    #[error("{}:{}: {} {} ({}:{})", problem_mark.line, problem_mark.column, problem, context, context_mark.line, context_mark.column)]
    Problem {
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    },
    #[error(transparent)]
    Reader(#[from] ReaderError),
}

#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("no more tokens")]
    UnexpectedEof,
    #[error("{}:{}: {}", mark.line, mark.column, problem)]
    Problem { problem: &'static str, mark: Mark },
    #[error("{}:{}: {} {} ({}:{})", mark.line, mark.column, problem, context, context_mark.line, context_mark.column)]
    ProblemWithContext {
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        mark: Mark,
    },
    #[error(transparent)]
    Scanner(#[from] ScannerError),
}

#[derive(Debug, thiserror::Error)]
pub enum ComposerError {
    #[error("{}:{}: {}", mark.line, mark.column, problem)]
    Problem { problem: &'static str, mark: Mark },
    #[error("{}:{}: {} {} ({}:{})", mark.line, mark.column, problem, context, context_mark.line, context_mark.column)]
    ProblemWithContext {
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        mark: Mark,
    },
    #[error(transparent)]
    Parser(#[from] ParserError),
}
