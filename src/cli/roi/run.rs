use std::cell::RefCell;

use std::ops::DerefMut;

use clap::ArgMatches;
use indicatif::ProgressBar;
use itertools::Itertools;
use rayon::prelude::*;

use crate::cli::shared;
use crate::cli::shared::args::CoreArgs;
use crate::cli::shared::stranding::Stranding;
use crate::cli::shared::thread_cache::ThreadCache;
use crate::core::counting::buffers::ROIBuffer;
use crate::core::counting::nuccounter::{BaseNucCounter, StrandedNucCounter};
use crate::core::filtering::summary::ROISummaryFilter;
use crate::core::runner::BaseRunner;
use crate::core::runner::Runner;
use crate::core::stats::{EditingIndex, ROIBasedStat};
use crate::core::stranding::deduct::DeductStrandByDesign;
use crate::core::stranding::predict::ROIStrandPredictor;
use crate::core::summary::ROISummary;
use crate::core::workload::ROIWorkload;

use super::args::ROIArgs;
use super::resformat;

pub fn run(args: &ArgMatches, mut core: CoreArgs, factory: impl Fn() -> ProgressBar) {
    let args = ROIArgs::new(&core, args, &factory);
    let statbuilder = args.ei.as_ref().map(|_| EditingIndex::default());

    // Callbacks to track progress
    let pbar = factory();
    pbar.set_style(shared::style::pbar());
    pbar.set_draw_delta((core.threads * 10) as u64);
    pbar.set_length(args.workload.len() as u64);

    let oniter = |_: &[ROISummary]| pbar.inc(1);
    let onfinish = |intervals: &[ROISummary], reads: u32| {
        pbar.finish_with_message(format!("Finished with {} regions, total mapped reads: {}", intervals.len(), reads))
    };

    let (summary, ei) = match core.stranding {
        Stranding::Unstranded => {
            let counter = BaseNucCounter::new(core.readfilter, ROIBuffer::new(args.maxwsize as usize));
            let runner = BaseRunner::new(core.bamfiles, core.reference, counter, core.refnucpred);
            process(args.workload, runner, args.strandpred, args.outfilter, statbuilder, oniter, onfinish)
        }
        Stranding::Stranded(design) => {
            let deductor = DeductStrandByDesign::new(design);
            let forward = BaseNucCounter::new(core.readfilter.clone(), ROIBuffer::new(args.maxwsize as usize));
            let reverse = BaseNucCounter::new(core.readfilter, ROIBuffer::new(args.maxwsize as usize));
            let counter = StrandedNucCounter::new(forward, reverse, deductor);
            let runner = BaseRunner::new(core.bamfiles, core.reference, counter, core.refnucpred);
            process(args.workload, runner, args.strandpred, args.outfilter, statbuilder, oniter, onfinish)
        }
    };

    resformat::regions(&mut core.saveto, summary);
    match (ei, args.ei) {
        (Some(ei), Some(mut writer)) => {
            resformat::statistic(&core.name, &mut writer, &ei);
        }
        (_, _) => {}
    }
}

fn process<
    Rnr: Runner + Clone + Send,
    StrandPred: ROIStrandPredictor + Clone + Send,
    Filter: ROISummaryFilter + Clone + Send,
    StatBuilder: ROIBasedStat + Clone + Send,
    OnIteration: Fn(&[ROISummary]) + Sync,
    OnFinish: FnOnce(&[ROISummary], u32),
>(
    workload: Vec<ROIWorkload>,
    runner: Rnr,
    strandpred: StrandPred,
    filter: Filter,
    statbuilder: Option<StatBuilder>,
    oniter: OnIteration,
    onfinish: OnFinish,
) -> (Vec<ROISummary>, Option<StatBuilder>) {
    let dostat = statbuilder.is_some();
    let ctxstore = ThreadCache::new(move || {
        RefCell::new((runner.clone(), strandpred.clone(), filter.clone(), statbuilder.clone()))
    });

    let results: Vec<ROISummary> = workload
        .into_par_iter()
        .map(|w| {
            let mut ctx = ctxstore.get().borrow_mut();
            let (rnr, strandpred, filter, stat) = ctx.deref_mut();
            let result = doiter(w, rnr, strandpred, filter, stat);
            oniter(&result);
            result
        })
        .flatten()
        .collect();

    let ctxstore = ctxstore.dissolve().map(|x| x.into_inner()).collect_vec();

    let mapped = ctxstore.iter().map(|x| x.0.mapped()).sum();

    let stats = if dostat {
        let buffer = ctxstore.into_iter().map(|x| x.3.unwrap()).collect_vec();
        Some(StatBuilder::combine(buffer.as_slice()))
    } else {
        None
    };

    onfinish(&results, mapped);

    (results, stats)
}

fn doiter(
    w: ROIWorkload,
    rnr: &mut impl Runner,
    strandpred: &impl ROIStrandPredictor,
    filter: &impl ROISummaryFilter,
    stat: &mut Option<impl ROIBasedStat>,
) -> Vec<ROISummary> {
    // Count nucleotides and infer the reference sequence
    let mut results = rnr
        .run(w)
        .into_iter()
        .map(|x| {
            let mut summary: ROISummary = x.into();

            if summary.strand.is_unknown() {
                summary.strand = strandpred.predict(&summary.interval, &summary.mismatches);
            }
            summary
        })
        .collect_vec();

    if let Some(stat) = stat {
        for x in &results {
            stat.process(x);
        }
    }

    results.retain(|x| filter.is_ok(x));
    results
}

// #[cfg(test)]
// mod test {
//     // use bio_types::genome::Interval;
//     // use mockall::predicate::*;
//     // use mockall::Sequence;
//     //
//     // use crate::core::counting::{CountsBufferContent, NucCounterContent, NucCounts};
//     // use crate::core::dna::Nucleotide;
//     // use crate::core::filtering::summary::MockIntervalSummaryFilter;
//     // use crate::core::run::runner::MockROIRunCtx;
//     // use crate::core::stats::MockIntervalBasedStat;
//     // use crate::core::stranding::predict::MockIntervalStrandPredictor;
//     //
//     // use super::*;
//     //
//     // const FORWARD: &'static [NucCounts] =
//     //     &[NucCounts { A: 1, C: 2, G: 12, T: 0 }, NucCounts { A: 0, C: 10, G: 0, T: 0 }];
//     // const REVERSE: &'static [NucCounts] = &[NucCounts { A: 1, C: 0, G: 0, T: 0 }, NucCounts { A: 0, C: 0, G: 0, T: 1 }];
//     // const UNSTRANDED: &'static [NucCounts] = &[
//     //     NucCounts { A: 1, C: 0, G: 0, T: 0 },
//     //     NucCounts { A: 0, C: 1, G: 0, T: 0 },
//     //     NucCounts { A: 0, C: 0, G: 2, T: 0 },
//     //     NucCounts { A: 0, C: 0, G: 2, T: 3 },
//     // ];
//     //
//     // fn mock_strandpred(returning: Strand) -> MockIntervalStrandPredictor {
//     //     let mut mock = MockIntervalStrandPredictor::new();
//     //     mock.expect_predict().once().return_const(returning);
//     //     mock
//     // }
//     //
//     // fn mock_filter(returning: bool) -> MockIntervalSummaryFilter {
//     //     let mut mock = MockIntervalSummaryFilter::new();
//     //     mock.expect_is_ok().once().return_const(returning);
//     //     mock
//     // }
//     //
//     // #[test]
//     // fn run_empty() {
//     //     let mut runner = MockROIRunCtx::new();
//     //     let w = Workload { interval: Interval::new("chr".into(), 1..2), name: "".into() };
//     //
//     //     // Empty result
//     //     let mut seq = Sequence::new();
//     //     runner.expect_count().once().with(eq(w.interval.clone())).in_sequence(&mut seq).return_const(());
//     //     runner.expect_finalize()
//     //         .once()
//     //         .in_sequence(&mut seq)
//     //         .returning(|| Option::<(NucCounterContent, Vec<Nucleotide>)>::None);
//     //     let mut stat = MockIntervalBasedStat::new();
//     //     assert!(super::_run(&w, &mut runner, Some(&mut stat)).is_empty());
//     // }
//     //
//     // #[test]
//     // fn run_stranded() {
//     //     let mut runner = MockROIRunCtx::new();
//     //     let w = Workload { interval: Interval::new("chr".into(), 1..2), name: "".into() };
//     //
//     //     // Stranded results
//     //     let mut seq = Sequence::new();
//     //     runner.expect_count().once().with(eq(w.interval.clone())).in_sequence(&mut seq).return_const(());
//     //
//     //     let content = NucCounterContent {
//     //         interval: w.interval.clone(),
//     //         counts: CountsBufferContent { forward: Some(FORWARD), reverse: Some(REVERSE), unstranded: None },
//     //     };
//     //
//     //     runner.expect_finalize()
//     //         .once()
//     //         .in_sequence(&mut seq)
//     //         .return_const(Some((content, vec![Nucleotide::G, Nucleotide::T])));
//     //     let mut stat = MockIntervalBasedStat::new();
//     //     stat.expect_process().times(2).in_sequence(&mut seq).return_const(());
//     //     for filter in [true, false] {
//     //         runner.expect_filter().once().in_sequence(&mut seq).return_const(mock_filter(filter));
//     //     }
//     //     let result = super::_run(&w, &mut runner, Some(&mut stat));
//     //     assert_eq!(result.len(), 1);
//     //     assert!(!result[0].strand.is_unknown());
//     // }
//     //
//     // #[test]
//     // fn run_unstranded() {
//     //     let mut runner = MockROIRunCtx::new();
//     //     let w = Workload { interval: Interval::new("chr".into(), 1..2), name: "".into() };
//     //
//     //     // Unstranded results
//     //     let mut seq = Sequence::new();
//     //     runner.expect_count().once().with(eq(w.interval.clone())).in_sequence(&mut seq).return_const(());
//     //
//     //     let content = NucCounterContent {
//     //         interval: w.interval.clone(),
//     //         counts: CountsBufferContent { forward: None, reverse: None, unstranded: Some(UNSTRANDED) },
//     //     };
//     //     runner.expect_finalize()
//     //         .once()
//     //         .in_sequence(&mut seq)
//     //         .return_const(Some((content, vec![Nucleotide::G, Nucleotide::T, Nucleotide::A, Nucleotide::C])));
//     //     runner.expect_strandpred().once().in_sequence(&mut seq).return_const(mock_strandpred(Strand::Forward));
//     //     let mut stat = MockIntervalBasedStat::new();
//     //     stat.expect_process().once().in_sequence(&mut seq).return_const(());
//     //     runner.expect_filter().once().in_sequence(&mut seq).return_const(mock_filter(true));
//     //
//     //     let result = super::_run(&w, &mut runner, Some(&mut stat));
//     //     assert_eq!(result.len(), 1);
//     //     assert_eq!(result[0].strand, Strand::Forward);
//     // }
// }