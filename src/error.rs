#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o")]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
