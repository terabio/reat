use std::marker::PhantomData;

use crate::core::counting::buffers::CountsBufferContent;
use crate::core::read::AlignedRead;

use super::{CountsBuffer, NucCounts};

#[derive(Clone)]
pub struct UnstrandedCountsBuffer<R: AlignedRead> {
    buffer: Vec<NucCounts>,
    phantom: PhantomData<R>,
}

impl<R: AlignedRead> UnstrandedCountsBuffer<R> {
    pub fn new(reserve: u32) -> Self {
        let buffer = Vec::with_capacity(reserve as usize);
        UnstrandedCountsBuffer { buffer, phantom: Default::default() }
    }
}

impl<R: AlignedRead> CountsBuffer<R> for UnstrandedCountsBuffer<R> {
    fn reset(&mut self, newlen: u32) {
        let newlen = newlen as usize;
        if self.buffer.len() != newlen {
            self.buffer.resize(newlen, NucCounts::zeros());
        }
        self.buffer.fill(NucCounts::zeros());
    }

    #[inline]
    fn buffer_for(&mut self, _: &mut R) -> &mut [NucCounts] {
        &mut self.buffer
    }

    #[inline]
    fn content(&self) -> CountsBufferContent {
        CountsBufferContent { forward: None, reverse: None, unstranded: Some(&self.buffer) }
    }

    #[inline]
    fn len(&self) -> u32 {
        self.buffer.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;
    use crate::core::read::MockRead;

    fn dummy(reserve: u32) -> UnstrandedCountsBuffer<MockRead> {
        UnstrandedCountsBuffer::new(reserve)
    }

    #[test]
    fn reset() {
        let mut dummy = dummy(10);
        assert_eq!(dummy.len(), 0);
        for x in [20, 10, 5] {
            dummy.reset(x);
            assert_eq!(dummy.len(), x);
            // previous changes must be cleaned
            assert!(dummy.buffer.iter().all(|x| x.coverage() == 0), "{:?}", dummy.buffer);
            // new dummy changes
            dummy.buffer[0].T = 100;
        }
    }

    #[test]
    fn buffer_for() {
        let mut dummy = dummy(2);
        let (mut forward, mut reverse) = (MockRead::new(), MockRead::new());
        assert!(ptr::eq(dummy.buffer_for(&mut forward), dummy.buffer_for(&mut reverse)));
    }

    #[test]
    fn content() {
        let mut dummy = dummy(10);
        dummy.reset(10);
        dummy.buffer[0].A = 10;

        let content = dummy.content();
        assert!(content.forward.is_none() && content.reverse.is_none());
        assert!(content.unstranded.is_some() && content.unstranded.unwrap().len() == 10);
    }
}
