use std::fs::File;

use clap::Arg;
use clap::ArgMatches;
use indicatif::ProgressBar;

use crate::cli::shared;
use crate::cli::shared::validate;
use crate::core::mismatches::prefilters;
use crate::core::mismatches::prefilters::retain::RetainROIFromList;
use crate::core::mismatches::roi::ROIMismatchesVec;
use crate::core::stranding::predict::REATStrandingEngine;
use crate::core::workload::ROIWorkload;

use super::parse;

pub mod stats {
    use super::*;

    pub const EDITING_INDEX: &str = "ei";

    pub const SECTION_NAME: &str = "Stats";

    pub fn args<'a>() -> Vec<Arg<'a>> {
        let args = vec![Arg::new(EDITING_INDEX)
            .long(EDITING_INDEX)
            .takes_value(true)
            .validator(validate::writable)
            .long_help(
                "File for saving Editing Indexes (EI). \
                If the file already exists, EI for the current experiments will be appended to it",
            )];
        args.into_iter().map(|x| x.help_heading(Some(SECTION_NAME))).collect()
    }
}

pub mod special {
    use super::*;

    pub const ROI: &str = "rois";

    pub const SECTION_NAME: &str = "Special information";

    pub fn args<'a>() -> Vec<Arg<'a>> {
        let args = vec![Arg::new(ROI).long(ROI).required(true).takes_value(true).validator(validate::path).long_help(
            "Path to a BED file with regions of interest(ROIS) \
            with at least 4 first BED columns(chr, start, end, name)",
        )];
        args.into_iter().map(|x| x.help_heading(Some(SECTION_NAME))).collect()
    }
}

pub mod output_filtering {
    use super::*;

    pub const MIN_MISMATCHES: &str = "out-min-mismatches";
    pub const MIN_FREQ: &str = "out-min-freq";
    pub const MIN_COVERAGE: &str = "out-min-cov";
    pub const FORCE_LIST: &str = "force";

    pub const SECTION_NAME: &str = "Output hooks";

    pub fn args<'a>() -> Vec<Arg<'a>> {
        let args = vec![
            Arg::new(MIN_COVERAGE)
                .long(MIN_COVERAGE)
                .takes_value(true)
                .validator(validate::numeric(0u32, u32::MAX))
                .default_value("10")
                .long_help("Output only ROIs covered by at least X unique filters(after filters/bases hooks)"),
            Arg::new(MIN_MISMATCHES)
                .long(MIN_MISMATCHES)
                .takes_value(true)
                .validator(validate::numeric(0u32, u32::MAX))
                .default_value("5")
                .long_help(
                    "Output only ROI having total number of mismatches ≥ threshold. \
                    Mismatches are counted jointly, i.e. for the \"A\" reference we have \"C\" + \"G\" + \"T\". \
                    For \"N\" reference all nucleotides are considered as mismatches. \
                    This is a deliberate choice to allow a subsequent user to work through / filter such records",
                ),
            Arg::new(MIN_FREQ)
                .long(MIN_FREQ)
                .takes_value(true)
                .validator(validate::numeric(0f32, 1f32))
                .default_value("0.01")
                .long_help(
                    "Output only ROI having total mismatches frequency ≥ threshold (freq = ∑ mismatches / coverage)",
                ),
            Arg::new(FORCE_LIST).long(FORCE_LIST).takes_value(true).validator(validate::path).long_help(
                "Force the output of ROIs located in a given BED file (even if they do not pass other filters).",
            ),
        ];
        args.into_iter().map(|x| x.help_heading(Some(SECTION_NAME))).collect()
    }
}

pub fn all<'a>() -> Vec<Arg<'a>> {
    shared::args::all()
        .into_iter()
        .chain(stats::args())
        .chain(special::args())
        .chain(output_filtering::args())
        .collect()
}

pub struct ROIArgs {
    pub workload: Vec<ROIWorkload>,
    pub maxwsize: usize,
    pub prefilter: prefilters::ByMismatches,
    pub ei: Option<(String, csv::Writer<File>)>,
    pub stranding: REATStrandingEngine<ROIMismatchesVec>,
    pub retain: Option<RetainROIFromList>,
}

impl ROIArgs {
    pub fn new(core: &shared::args::CoreArgs, args: &ArgMatches, factory: &impl Fn() -> ProgressBar) -> Self {
        let prefilter = shared::parse::outfilter(
            factory(),
            output_filtering::MIN_MISMATCHES,
            output_filtering::MIN_FREQ,
            output_filtering::MIN_COVERAGE,
            args,
        );
        let ei = parse::editing_index(factory(), args);

        let mut stranding = REATStrandingEngine::new();
        let mut workload: Option<Vec<ROIWorkload>> = Default::default();
        let mut maxsize: Option<usize> = Default::default();
        let mut retain: Option<RetainROIFromList> = Default::default();

        let (pbarw, pbars, pbarr) = (factory(), factory(), factory());
        let excluded = core.excluded.clone();
        rayon::scope(|s| {
            s.spawn(|_| {
                let (w, m) = parse::work(pbarw, args, excluded);
                workload = Some(w);
                maxsize = Some(m)
            });
            s.spawn(|_| stranding = shared::parse::strandpred(pbars, args));
            s.spawn(|_| retain = parse::retain(pbarr, args));
        });

        Self { workload: workload.unwrap(), maxwsize: maxsize.unwrap(), prefilter, ei, stranding, retain }
    }
}
