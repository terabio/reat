#[cfg(test)]
use mockall::{automock, predicate::*};

use crate::core::read::AlignedRead;
pub use locus::NucCounts;
pub use strandedbuf::StrandedCountsBuffer;
pub use unstrandedbuf::UnstrandedCountsBuffer;

mod locus;
mod strandedbuf;
mod unstrandedbuf;

#[derive(Copy, Clone)]
pub struct CountsBufferContent<'a> {
    pub forward: Option<&'a [NucCounts]>,
    pub reverse: Option<&'a [NucCounts]>,
    pub unstranded: Option<&'a [NucCounts]>,
}

impl<'a> CountsBufferContent<'a> {
    pub fn total_counts(&self) -> usize {
        self.forward.map_or(0, |x| x.len())
            + self.reverse.map_or(0, |x| x.len())
            + self.unstranded.map_or(0, |x| x.len())
    }
}

#[cfg_attr(test, automock)]
pub trait CountsBuffer<R: AlignedRead> {
    fn reset(&mut self, len: u32);
    // record must NOT be mutable here, yet some const(!) methods in rust_htslib require mutable(!) instance
    fn buffer_for(&mut self, record: &mut R) -> &mut [NucCounts];
    fn content(&'_ self) -> CountsBufferContent<'_>;
    fn len(&self) -> u32;
}
