use bio_types::strand::{Same, Strand};
use derive_getters::Getters;
use derive_more::Constructor;

use crate::core::dna::{NucCounts, Nucleotide};
use crate::core::mismatches::roi::ROIDataRef;
use crate::core::mismatches::roi::{ROIMismatchesVec, ROINucCounts};
use crate::core::mismatches::site::{SiteDataRef, SiteMismatchesVec};
use crate::core::refpred::PredNucleotide;
use crate::core::stranding::predict::algo::utils;
use crate::core::stranding::predict::StrandingAlgo;
use crate::core::strandutil::Stranded;

#[derive(Constructor, Getters, Copy, Clone)]
pub struct StrandByAtoIEditing {
    minmismatches: u32,
    minfreq: f32,
}

impl StrandByAtoIEditing {
    #[inline]
    fn edited(&self, matches: f32, mismatches: f32) -> bool {
        let coverage = mismatches + matches;
        coverage > f32::EPSILON && mismatches >= self.minmismatches as f32 && (mismatches / coverage) >= self.minfreq
    }

    #[inline]
    fn sitepred(&self, sequenced: &NucCounts, refnuc: Nucleotide) -> Strand {
        match refnuc {
            Nucleotide::A => {
                if self.edited(sequenced.A as f32, sequenced.G as f32) {
                    Strand::Forward
                } else {
                    Strand::Unknown
                }
            }
            Nucleotide::T => {
                if self.edited(sequenced.T as f32, sequenced.C as f32) {
                    Strand::Reverse
                } else {
                    Strand::Unknown
                }
            }
            Nucleotide::C | Nucleotide::G | Nucleotide::Unknown => Strand::Unknown,
        }
    }

    #[inline]
    fn roipred(&self, mismatches: &ROINucCounts) -> Strand {
        let a2g = self.edited(mismatches.A.A, mismatches.A.G);
        let t2c = self.edited(mismatches.T.T, mismatches.T.C);

        match (a2g, t2c) {
            (false, false) => Strand::Unknown,
            (false, true) => Strand::Reverse,
            (true, false) => Strand::Forward,
            (true, true) => {
                let a2g_coverage = mismatches.A.A + mismatches.A.G;
                let t2c_coverage = mismatches.T.T + mismatches.T.C;

                if a2g_coverage <= f32::EPSILON && t2c_coverage <= f32::EPSILON {
                    Strand::Unknown
                } else {
                    let a2g = mismatches.A.G as f32 / a2g_coverage as f32;
                    let t2c = mismatches.T.C as f32 / t2c_coverage as f32;
                    if mismatches.A.G > f32::EPSILON && a2g > t2c {
                        Strand::Forward
                    } else if mismatches.T.C > f32::EPSILON && t2c > a2g {
                        Strand::Reverse
                    } else {
                        Strand::Unknown
                    }
                }
            }
        }
    }
}

impl StrandingAlgo<ROIMismatchesVec> for StrandByAtoIEditing {
    fn predict(&self, _: &str, items: &mut Stranded<ROIMismatchesVec>) {
        utils::assort_strands!(items, |x: ROIDataRef| self.roipred(x.mismatches));
    }
}

impl StrandingAlgo<SiteMismatchesVec> for StrandByAtoIEditing {
    fn predict(&self, _: &str, items: &mut Stranded<SiteMismatchesVec>) {
        utils::assort_strands!(items, |x: SiteDataRef| match x.prednuc {
            PredNucleotide::Homozygous(nuc) => self.sitepred(x.sequenced, *nuc),
            PredNucleotide::Heterozygous((n1, n2)) => {
                let (s1, s2) = (self.sitepred(x.sequenced, *n1), self.sitepred(x.sequenced, *n2));
                match (s1.is_unknown(), s2.is_unknown()) {
                    (false, true) => s1,
                    (true, false) => s2,
                    (false, false) if s1.same(&s2) => s1,
                    _ => Strand::Unknown,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Neg;

    use bio_types::strand::Same;

    use super::*;

    #[test]
    fn roi_strand_by_editing() {
        let dummy = StrandByAtoIEditing::new(8, 0.05);
        for (result, matches, mismatches) in
            [(Strand::Forward, 8, 8), (Strand::Unknown, 100, 4), (Strand::Unknown, 1, 7), (Strand::Forward, 10, 10)]
        {
            let (matches, mismatches) = (matches as f32, mismatches as f32);
            let mut mm = ROINucCounts::zeros();
            mm.T.T = matches;
            mm.T.C = mismatches;
            assert!(result.neg().same(&dummy.roipred(&mm)));

            mm = ROINucCounts::zeros();
            mm.A.A = matches;
            mm.A.G = mismatches;
            assert!(result.same(&dummy.roipred(&mm)));
        }

        for (result, matches, a2g, t2c) in
            [(Strand::Unknown, 10, 10, 10), (Strand::Reverse, 10, 10, 11), (Strand::Forward, 10, 11, 10)]
        {
            let (matches, a2g, t2c) = (matches as f32, a2g as f32, t2c as f32);

            let mut mm = ROINucCounts::zeros();
            mm.A.A = matches;
            mm.A.G = a2g;

            mm.T.T = matches;
            mm.T.C = t2c;

            assert!(result.same(&dummy.roipred(&mm)));
        }
    }

    #[test]
    fn nucpred() {
        let dummy = StrandByAtoIEditing::new(10, 0.1);

        // Unknown strand
        for (sequenced, refnuc) in [
            (NucCounts::T(123), Nucleotide::C),
            (NucCounts::G(234), Nucleotide::Unknown),
            (NucCounts::A(32), Nucleotide::G),
            (NucCounts::new(170, 0, 170, 0), Nucleotide::C),
            (NucCounts::new(10, 0, 9, 0), Nucleotide::A),
            (NucCounts::new(0, 200, 0, 200), Nucleotide::G),
        ] {
            assert!(dummy.sitepred(&sequenced, refnuc).is_unknown());
        }

        let dummy = StrandByAtoIEditing::new(8, 0.05);
        // Inferred strand
        for (matches, mismatches, strand) in
            [(8, 8, Strand::Forward), (100, 4, Strand::Unknown), (1, 7, Strand::Unknown), (10, 10, Strand::Forward)]
        {
            let cnts = NucCounts::new(matches, 0, mismatches, 0);
            assert!(dummy.sitepred(&cnts, Nucleotide::A).same(&strand));

            let cnts = NucCounts::new(0, mismatches, 0, matches);
            assert!(dummy.sitepred(&cnts, Nucleotide::T).same(&strand.neg()));
        }
    }
}
