#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GridError {
    WrongTileCount { expected: usize, actual: usize },
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::WrongTileCount { expected, actual } => {
                write!(f, "tile grid expected {expected} tiles, got {actual}")
            }
        }
    }
}

impl std::error::Error for GridError {}
