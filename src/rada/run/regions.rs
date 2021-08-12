use std::path::Path;

use bio_types::strand::{Same, Strand};
use rust_htslib::bam::record::Record;

use crate::rada::counting::NucCounter;
use crate::rada::filtering::summary::IntervalSummaryFilter;
use crate::rada::refnuc::RefNucPredictor;
use crate::rada::stranding::predict::IntervalStrandPredictor;
use crate::rada::summary::IntervalSummary;
use crate::rada::workload::Workload;

use super::context::ThreadContext;

pub fn run<
    Counter: NucCounter<Record>,
    RefNucPred: RefNucPredictor,
    StrandPred: IntervalStrandPredictor,
    Filter: IntervalSummaryFilter,
>(
    workload: Vec<Workload>,
    bamfiles: &[&Path],
    reference: &Path,
    counter: Counter,
    refnucpred: RefNucPred,
    strandpred: StrandPred,
    filter: Filter,
) -> Vec<IntervalSummary> {
    let mut ctx = ThreadContext::new(bamfiles, reference, counter, refnucpred, strandpred, filter);
    workload
        .into_iter()
        .map(|w| {
            // Counting nucleotides occurrence
            ctx.nuccount(&w.interval);
            let content = ctx.counter.content();
            let counts = &content.counts;

            if counts.forward.is_none() && counts.reverse.is_none() && counts.unstranded.is_none() {
                return vec![];
            }

            // Fetch reference sequence and predict "real" sequence based on the sequenced nucleotides
            let sequence = ctx.predseq(&ctx.reference(&content.interval), &content.counts);

            // Build summaries
            let mut result = Vec::with_capacity(3);
            for (strand, counts) in [
                (Strand::Forward, counts.forward),
                (Strand::Reverse, counts.reverse),
                (Strand::Unknown, counts.unstranded),
            ] {
                if let Some(counts) = counts {
                    let mut summary = IntervalSummary::from_counts(
                        content.interval.clone(),
                        w.name.clone(),
                        strand,
                        &sequence,
                        counts,
                    );
                    if strand.same(&Strand::Unknown) {
                        summary.strand = ctx.strandpred.predict(&summary.interval, &summary.mismatches);
                    }
                    result.push(summary);
                }
            }

            // Filter results
            result.retain(|x| ctx.filter.is_ok(x));

            result
        })
        .flatten()
        .collect()
}
