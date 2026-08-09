use std::fmt::{Debug, Formatter};
use std::io::{self, Read};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const OUTPUT_BUFFER_SIZE: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum CoreOutputEvent {
    Chunk {
        stream: CoreOutputStream,
        bytes: Vec<u8>,
    },
    ReadFailed {
        stream: CoreOutputStream,
        source: io::Error,
    },
}

pub struct CoreOutput {
    receiver: Receiver<CoreOutputEvent>,
}

impl Debug for CoreOutput {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CoreOutput")
    }
}

impl CoreOutput {
    /// Waits up to `timeout` for the next stdout or stderr event.
    ///
    /// # Errors
    ///
    /// Returns [`RecvTimeoutError::Timeout`] when no event arrives before the
    /// timeout, or [`RecvTimeoutError::Disconnected`] after all readers exit.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<CoreOutputEvent, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

pub(crate) struct CoreOutputReader {
    stream: CoreOutputStream,
    handle: JoinHandle<()>,
}

impl CoreOutputReader {
    pub(crate) fn join(self) -> Result<(), CoreOutputStream> {
        self.handle.join().map_err(|_| self.stream)
    }
}

pub(crate) fn output_channel() -> (Sender<CoreOutputEvent>, CoreOutput) {
    let (sender, receiver) = mpsc::channel();
    (sender, CoreOutput { receiver })
}

pub(crate) fn spawn_output_reader<R>(
    stream: CoreOutputStream,
    reader: R,
    sender: Sender<CoreOutputEvent>,
) -> io::Result<CoreOutputReader>
where
    R: Read + Send + 'static,
{
    let handle = thread::Builder::new()
        .name(format!("magies-core-{stream:?}"))
        .spawn(move || read_output(stream, reader, &sender))?;
    Ok(CoreOutputReader { stream, handle })
}

fn read_output(stream: CoreOutputStream, mut reader: impl Read, sender: &Sender<CoreOutputEvent>) {
    let mut buffer = [0_u8; OUTPUT_BUFFER_SIZE];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return,
            Ok(length) => {
                let _ = sender.send(CoreOutputEvent::Chunk {
                    stream,
                    bytes: buffer[..length].to_vec(),
                });
            }
            Err(source) => {
                let message = source.to_string();
                if sender
                    .send(CoreOutputEvent::ReadFailed { stream, source })
                    .is_err()
                {
                    eprintln!("failed to read Core {stream:?}: {message}");
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read denied",
            ))
        }
    }

    #[test]
    fn emits_the_original_typed_read_error() {
        let (sender, output) = output_channel();
        assert_eq!(format!("{output:?}"), "CoreOutput");

        read_output(CoreOutputStream::Stderr, FailingReader, &sender);

        match output.recv_timeout(Duration::from_millis(10)).unwrap() {
            CoreOutputEvent::ReadFailed { stream, source } => {
                assert_eq!(stream, CoreOutputStream::Stderr);
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
                assert_eq!(source.to_string(), "read denied");
            }
            event @ CoreOutputEvent::Chunk { .. } => {
                panic!("expected a read failure, got {event:?}")
            }
        }
    }

    #[test]
    fn handles_a_read_error_after_the_receiver_is_dropped() {
        let (sender, output) = output_channel();
        drop(output);

        read_output(CoreOutputStream::Stdout, FailingReader, &sender);
    }
}
