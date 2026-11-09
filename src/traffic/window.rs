//! 60-bucket one-second rolling ring and integer rate math (std only).

use super::BUCKETS;

/// One second of traffic (32 bytes; counts saturate at `u32::MAX`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bucket {
    pub rx: u64,
    pub tx: u64,
    pub msgs: u32,
    pub esc: u32,
    pub events: u32,
}

/// Direction selector for series access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Rx,
    Tx,
}

/// Rolling averages over 1 s / 10 s / 1 m (bytes per second, integer math).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rates {
    pub rx_1: u64,
    pub rx_10: u64,
    pub rx_60: u64,
    pub tx_1: u64,
    pub tx_10: u64,
    pub tx_60: u64,
    /// 10 s averages.
    pub msgs_10: u64,
    pub events_10: u64,
    pub esc_10: u64,
}

impl Rates {
    pub fn add(&mut self, o: &Rates) {
        self.rx_1 += o.rx_1;
        self.rx_10 += o.rx_10;
        self.rx_60 += o.rx_60;
        self.tx_1 += o.tx_1;
        self.tx_10 += o.tx_10;
        self.tx_60 += o.tx_60;
        self.msgs_10 += o.msgs_10;
        self.events_10 += o.events_10;
        self.esc_10 += o.esc_10;
    }
}

/// Fixed-size ring of the last [`BUCKETS`] seconds.
#[derive(Clone, Debug)]
pub struct Ring {
    buckets: [Bucket; BUCKETS],
    /// Index the next push writes to.
    head: usize,
    filled: usize,
    peak_rx: u64,
    peak_tx: u64,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    pub fn new() -> Self {
        Self {
            buckets: [Bucket::default(); BUCKETS],
            head: 0,
            filled: 0,
            peak_rx: 0,
            peak_tx: 0,
        }
    }

    /// Number of seconds of history held (saturates at [`BUCKETS`]).
    pub fn filled(&self) -> usize {
        self.filled
    }

    pub fn push_second(&mut self, b: Bucket) {
        self.buckets[self.head] = b;
        self.head = (self.head + 1) % BUCKETS;
        self.filled = (self.filled + 1).min(BUCKETS);
        self.peak_rx = self.peak_rx.max(b.rx);
        self.peak_tx = self.peak_tx.max(b.tx);
    }

    /// Bucket `age` seconds ago (0 = newest). Caller guarantees `age < filled`.
    fn at(&self, age: usize) -> &Bucket {
        &self.buckets[(self.head + BUCKETS - 1 - age) % BUCKETS]
    }

    fn avg(&self, window: usize, f: impl Fn(&Bucket) -> u64) -> u64 {
        let n = window.min(self.filled);
        if n == 0 {
            return 0;
        }
        (0..n).map(|a| f(self.at(a))).sum::<u64>() / n as u64
    }

    pub fn rates(&self) -> Rates {
        Rates {
            rx_1: self.avg(1, |b| b.rx),
            rx_10: self.avg(10, |b| b.rx),
            rx_60: self.avg(60, |b| b.rx),
            tx_1: self.avg(1, |b| b.tx),
            tx_10: self.avg(10, |b| b.tx),
            tx_60: self.avg(60, |b| b.tx),
            msgs_10: self.avg(10, |b| u64::from(b.msgs)),
            events_10: self.avg(10, |b| u64::from(b.events)),
            esc_10: self.avg(10, |b| u64::from(b.esc)),
        }
    }

    /// Lifetime maximum single-second `(rx, tx)`.
    pub fn peak(&self) -> (u64, u64) {
        (self.peak_rx, self.peak_tx)
    }

    /// Maximum single-second `(rx, tx)` within the retained minute.
    pub fn peak_1m(&self) -> (u64, u64) {
        let mut p = (0, 0);
        for a in 0..self.filled {
            let b = self.at(a);
            p = (p.0.max(b.rx), p.1.max(b.tx));
        }
        p
    }

    /// Last `n` (clamped to 60) buckets of `dir`, oldest first, zero-padded on the left.
    pub fn series(&self, dir: Dir, n: usize) -> Vec<u64> {
        let n = n.min(BUCKETS);
        (0..n)
            .rev()
            .map(|age| {
                if age < self.filled {
                    let b = self.at(age);
                    match dir {
                        Dir::Rx => b.rx,
                        Dir::Tx => b.tx,
                    }
                } else {
                    0
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rx(v: u64) -> Bucket {
        Bucket { rx: v, ..Bucket::default() }
    }

    #[test]
    fn rates_hand_computed() {
        let mut r = Ring::new();
        for v in [1000, 0, 2000] {
            r.push_second(rx(v));
        }
        let t = r.rates();
        // Young channel: divides by 3 elapsed buckets, not 10/60.
        assert_eq!((t.rx_1, t.rx_10, t.rx_60), (2000, 1000, 1000));
        for _ in 0..7 {
            r.push_second(rx(100));
        }
        assert_eq!(r.rates().rx_10, (3000 + 700) / 10);
        assert_eq!(r.rates().rx_60, (3000 + 700) / 10);
        for _ in 0..50 {
            r.push_second(rx(60));
        }
        assert_eq!(r.rates().rx_60, (3000 + 700 + 3000) / 60);
    }

    #[test]
    fn peaks_and_series() {
        let mut r = Ring::new();
        for v in [5, 90, 7] {
            r.push_second(Bucket { rx: v, tx: v * 2, ..Bucket::default() });
        }
        assert_eq!(r.peak(), (90, 180));
        assert_eq!(r.peak_1m(), (90, 180));
        assert_eq!(r.series(Dir::Rx, 5), vec![0, 0, 5, 90, 7]);
        assert_eq!(r.series(Dir::Tx, 2), vec![180, 14]);
        for _ in 0..60 {
            r.push_second(Bucket::default());
        }
        assert_eq!(r.peak_1m(), (0, 0));
        assert_eq!(r.peak(), (90, 180), "lifetime peak survives rotation");
        assert_eq!(r.series(Dir::Rx, 100).len(), 60);
    }
}
