use crate::core::mismatches::roi::IntermediateROIMismatches;

use super::super::StrandingEngine;
use super::ROIStrandPredictor;

pub struct ROIStrandingEngine<T: IntermediateROIMismatches> {
    predictors: Vec<Box<dyn ROIStrandPredictor<T>>>,
}

impl<T: IntermediateROIMismatches> ROIStrandingEngine<T> {
    pub fn new() -> Self {
        Self { predictors: vec![] }
    }

    pub fn add(&mut self, predictor: impl ROIStrandPredictor<T> + 'static) {
        self.predictors.push(Box::new(predictor))
    }
}

impl<T: IntermediateROIMismatches> StrandingEngine<T> for ROIStrandingEngine<T> {
    fn strand(&self, items: Vec<T>) -> Vec<T> {
        let (mut unknown, mut result): (Vec<T>, Vec<T>) = items.into_iter().partition(|x| x.strand().is_unknown());
        result.reserve(result.len() + unknown.len());

        for predictor in &self.predictors {
            if unknown.is_empty() {
                break;
            }

            let (left, stranded) = predictor.predict(unknown).into_iter().partition(|x| x.strand().is_unknown());
            unknown = left;
            result.extend(stranded);
        }
        if !unknown.is_empty() {
            result.extend(unknown);
        }

        result
    }
}