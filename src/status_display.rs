use std::{
    io,
    sync::atomic::{AtomicU16, AtomicU64, Ordering},
    time::Duration,
};

use indicatif::ProgressBar;
use kanal::Sender;
use simple_moving_average::{SingleSumSMA, SMA};
use tokio::time;

use crate::threads::TrackedGroup;

pub static GROUPS_OWNED: AtomicU16 = AtomicU16::new(0);
pub static GROUPS_CLAIMED: AtomicU16 = AtomicU16::new(0);
pub static BATCH_CHECK_COUNTER: AtomicU64 = AtomicU64::new(0);
pub static BATCH_PROXIES: AtomicU64 = AtomicU64::new(0);
pub static ROBUX_CLAIMED: AtomicU64 = AtomicU64::new(0);

pub struct LogWriter(ProgressBar);
impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.suspend(|| io::stderr().write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.suspend(|| io::stderr().flush())
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.suspend(|| io::stderr().write_vectored(bufs))
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.suspend(|| io::stderr().write_all(buf))
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.0.suspend(|| io::stderr().write_fmt(fmt))
    }
}

impl LogWriter {
    pub const fn new(bar: ProgressBar) -> Self {
        Self(bar)
    }
}

pub async fn status_thread(
    bar: ProgressBar,
    group_limit: u16,
    batch_senders: (Sender<TrackedGroup>, Sender<TrackedGroup>),
) {
    let mut batch: SingleSumSMA<u64, u64, 10> = SingleSumSMA::new();
    loop {
        batch.add_sample(BATCH_CHECK_COUNTER.swap(0, Ordering::Relaxed));

        bar.set_message(format!(
            "Groups claimed: {}\nRobux claimed: {}\nCPS: {}\nGroup capacity: {}/{}\nProxies left: {}\nQueue size: {}",
            GROUPS_CLAIMED.load(Ordering::Relaxed),
            ROBUX_CLAIMED.load(Ordering::Relaxed),
            batch.get_average(),
            GROUPS_OWNED.load(Ordering::Relaxed),
            group_limit,
            BATCH_PROXIES.load(Ordering::Relaxed),
            batch_senders.0.len() + batch_senders.1.len(),
        ));

        time::sleep(Duration::from_secs(1)).await;
    }
}
