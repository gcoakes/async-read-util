use async_read_utils::ObserveRead;
use futures::{io::Cursor, AsyncReadExt};
use rand::{thread_rng, Fill};

#[smol_potat::test]
async fn test_read() {
    let mut rng = thread_rng();
    let mut source = [0u8; 4096];
    source.try_fill(&mut rng).expect("fill with random numbers");
    let reader = Cursor::new(source);
    let mut copied = vec![];
    let mut observed = reader.observe(|chunk| copied.extend_from_slice(chunk));
    let mut collected = vec![];
    let mut buf = [0u8; 1024];
    loop {
        let nread = observed.read(&mut buf[..]).await.expect("able to read");
        if nread == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..nread]);
    }
    assert_eq!(collected, copied);
}
