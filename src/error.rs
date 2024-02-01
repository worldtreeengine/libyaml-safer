use crate::yaml_mark_t;

#[derive(Debug, thiserror::Error)]
pub enum EmitterError {
    #[error("{0}")]
    Problem(&'static str),
    #[error(transparent)]
    Writer(#[from] WriterError),
}

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error("writer could not flush the entire buffer")]
    Incomplete,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ReaderError {
    #[error("{problem}")]
    Problem {
        problem: &'static str,
        offset: u64,
        value: i32,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ScannerError {
    #[error("{problem}")]
    Problem {
        context: &'static str,
        context_mark: yaml_mark_t,
        problem: &'static str,
        problem_mark: yaml_mark_t,
    },
    #[error(transparent)]
    Reader(#[from] ReaderError),
}
