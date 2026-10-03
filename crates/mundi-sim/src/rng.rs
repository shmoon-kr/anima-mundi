//! The one source of chance (MECHANICS §17.1): rand(lo, hi) inclusive and dice(n, s), from the
//! seeded ChaCha. Same seed and inputs, same rolls.

use rand_core::Rng;

use crate::Sim;

impl Sim {
    /// A whole number from `lo` to `hi`, both included, uniformly; `lo` when `hi < lo`.
    pub(crate) fn rand(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo + 1) as u64;
        let zone = u64::MAX - u64::MAX % span;
        loop {
            let x = self.rng.next_u64();
            if x < zone {
                return lo + (x % span) as i64;
            }
        }
    }

    /// The sum of `n` rolls of `rand(1, s)`; 0 when either is not positive.
    pub(crate) fn dice(&mut self, n: i32, s: i32) -> i32 {
        if n <= 0 || s <= 0 {
            return 0;
        }
        (0..n).map(|_| self.rand(1, s as i64) as i32).sum()
    }
}
