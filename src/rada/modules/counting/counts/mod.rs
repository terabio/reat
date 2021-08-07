#[cfg(test)]
use mockall::{automock, predicate::*};
use rust_htslib::bam::record::Record;

use crate::rada::modules::read::AlignedRead;
pub use locus::LocusCounts;
pub use strandedbuf::StrandedCountsBuffer;
pub use unstrandedbuf::UnstrandedCountsBuffer;

mod locus;
mod strandedbuf;
mod unstrandedbuf;

#[derive(Copy, Clone)]
pub struct CountsBufferContent<'a> {
    pub forward: Option<&'a [LocusCounts]>,
    pub reverse: Option<&'a [LocusCounts]>,
    pub unstranded: Option<&'a [LocusCounts]>,
}

impl<'a> CountsBufferContent<'a> {
    pub fn capacity(&self) -> usize {
        self.forward.map_or(0, |x| x.len())
            + self.reverse.map_or(0, |x| x.len())
            + self.unstranded.map_or(0, |x| x.len())
    }
}

#[cfg_attr(test, automock)]
pub trait CountsBuffer<R: AlignedRead> {
    fn reset(&mut self, len: u32) -> &mut Self;
    // record must NOT be mutable here, yet some const(!) methods in rust_htslib require mutable(!) instance
    fn buffer_for(&mut self, record: &mut R) -> &mut [LocusCounts];
    fn content<'a>(&'a self) -> CountsBufferContent<'a>;
    fn len(&self) -> u32;
}
