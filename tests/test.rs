use async_read_util::AsyncReadUtil;
use futures::{io::Cursor, AsyncReadExt};
use rand::{thread_rng, Fill};
use std::cmp::min;

#[smol_potat::test]
async fn test_observe() {
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

#[smol_potat::test]
async fn test_map() {
    let mut rng = thread_rng();
    let mut source = [0u8; 4096];
    source.try_fill(&mut rng).expect("fill with random numbers");
    let reader = Cursor::new(source);

    let mut mapped_reader = reader.map_read(|src, dst| {
        for (x, y) in src.iter().zip(dst.iter_mut()) {
            *y = x / 2;
        }
        let nread = min(dst.len(), src.len());
        (nread, nread)
    });

    let mut collected = vec![];
    let mut buf = [0u8; 1024];
    loop {
        let nread = mapped_reader
            .read(&mut buf[..])
            .await
            .expect("able to read");
        if nread == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..nread]);
    }
    assert_eq!(collected, source.iter().map(|x| x / 2).collect::<Vec<u8>>());
}

#[smol_potat::test]
async fn test_map_expanding() {
    let mut rng = thread_rng();
    let mut source = [0u8; 4096];
    source.try_fill(&mut rng).expect("fill with random numbers");
    let reader = Cursor::new(source);

    let mut mapped_reader = reader.map_read(|src, dst| {
        let mut nread = 0;
        for (x, ys) in src.iter().zip(dst.chunks_mut(2)) {
            for y in ys {
                *y = *x;
            }
            nread += 1;
        }
        (nread, nread * 2)
    });

    let mut collected = vec![];
    let mut buf = [0u8; 1024];
    loop {
        let nread = mapped_reader
            .read(&mut buf[..])
            .await
            .expect("able to read");
        if nread == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..nread]);
    }
    assert_eq!(
        collected,
        source
            .iter()
            .flat_map(|x| vec![*x, *x])
            .collect::<Vec<u8>>()
    );
}

#[smol_potat::test]
async fn test_map_contracting() {
    let mut rng = thread_rng();
    let mut source = [0u8; 4096];
    source.try_fill(&mut rng).expect("fill with random numbers");
    let reader = Cursor::new(source);

    let mut mapped_reader = reader.map_read(|src, dst| {
        let mut nread = 0;
        for (xs, y) in src.chunks(2).zip(dst) {
            *y = *xs.iter().min().unwrap();
            nread += 1;
        }
        (nread * 2, nread)
    });

    let mut collected = vec![];
    let mut buf = [0u8; 1024];
    loop {
        let nread = mapped_reader
            .read(&mut buf[..])
            .await
            .expect("able to read");
        if nread == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..nread]);
    }
    assert_eq!(
        collected,
        source
            .chunks(2)
            .map(|xs| *xs.iter().min().unwrap())
            .collect::<Vec<u8>>()
    );
}
