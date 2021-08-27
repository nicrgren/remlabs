use std::{convert::TryFrom, error::Error as StdError, fmt, path};
use tokio::{fs::File, io::AsyncReadExt};

use crate::RawEvent;

#[derive(Debug)]
pub enum EventSourceError<E> {
    Io(std::io::Error),
    Parse(E),
}

impl<E> StdError for EventSourceError<E>
where
    E: StdError,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(inner) => inner.source(),
            Self::Parse(err) => err.source(),
        }
    }
}

impl<E> fmt::Display for EventSourceError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(inner) => write!(f, "Io error: {}", inner),
            Self::Parse(inner) => write!(f, "Parse error: {}", inner),
        }
    }
}

pub struct EventSource<T> {
    buf: [u8; 16],
    file: File,

    _marker: std::marker::PhantomData<T>,
}

impl<T> EventSource<T>
where
    T: TryFrom<RawEvent>,
{
    pub async fn open(path: impl AsRef<path::Path>) -> std::io::Result<EventSource<T>> {
        Ok(Self {
            buf: [0u8; 16],
            file: File::open(&path).await?,
            _marker: Default::default(),
        })
    }

    pub async fn next(&mut self) -> Result<T, EventSourceError<T::Error>> {
        if self
            .file
            .read_exact(&mut self.buf)
            .await
            .map_err(EventSourceError::Io)?
            != 16
        {
            return Err(EventSourceError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Read error. Could not read 16 bytes",
            )));
        }

        T::try_from(RawEvent::from(&self.buf)).map_err(EventSourceError::Parse)
    }
}
