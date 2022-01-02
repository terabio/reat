use crate::core::read::AlignedRead;

// #[cfg_attr(test, automock)]
// A function computed on top of sequenced filters in a given interval
pub trait ReadsCollider<'a, R: AlignedRead> {
    type ColliderResult;
    type Workload;

    fn reset(&mut self, info: Self::Workload);
    fn collide(&mut self, read: &R);
    fn finalize(&mut self);
    fn result(&'a self) -> Vec<Self::ColliderResult>;
}
