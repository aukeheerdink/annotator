use std::{cmp::Ordering, collections::HashSet, fmt::Write};

use itertools::Itertools;
use rustyms::{
    fragment::*,
    model::Location,
    modification::{CrossLinkName, Ontology, SimpleModification},
    placement_rule::PlacementRule,
    spectrum::{AnnotatedPeak, Fdr, PeakSpectrum, Recovered, Score},
    system::{da, mz, Mass, MassOverCharge},
    AnnotatedSpectrum, LinearPeptide, Linked, MassMode, Model, Modification, MolecularFormula,
    NeutralLoss,
};

use crate::html_builder::{HtmlContent, HtmlElement, HtmlTag};

use super::label::get_label;

pub fn annotated_spectrum(
    spectrum: &AnnotatedSpectrum,
    _id: &str,
    fragments: &[Fragment],
    model: &Model,
    mass_mode: MassMode,
) -> (String, Limits) {
    let mut output = String::new();
    let (limits, overview) = get_overview(spectrum);
    let (graph_data, graph_boundaries) =
        spectrum_graph_boundaries(spectrum, fragments, model, mass_mode);
    let multiple_peptidoforms = spectrum.peptide.peptidoforms().len() > 1;
    let multiple_peptides = spectrum
        .peptide
        .peptidoforms()
        .iter()
        .any(|p| p.peptides().len() > 1);
    let multiple_glycans = spectrum
        .peptide
        .peptidoforms()
        .iter()
        .flat_map(|p| p.peptides())
        .any(|p| {
            p.sequence
                .iter()
                .filter(|seq| {
                    seq.modifications.iter().any(|m| {
                        matches!(
                            m,
                            Modification::Simple(
                                SimpleModification::GlycanStructure(_)
                                    | SimpleModification::Glycan(_)
                                    | SimpleModification::Gno(_, _)
                            )
                        )
                    })
                })
                .count()
                > 1
        });
    let mut unique_peptide_lookup = Vec::new();
    render_peptide(
        &mut output,
        spectrum,
        overview,
        multiple_peptidoforms,
        multiple_peptides,
        &mut unique_peptide_lookup,
    );
    render_spectrum(
        &mut output,
        spectrum,
        fragments,
        &graph_boundaries,
        &limits,
        "first",
        multiple_peptidoforms,
        multiple_peptides,
        multiple_glycans,
        mass_mode,
        &unique_peptide_lookup,
    );
    // Error graph
    render_error_graph(
        &mut output,
        &graph_boundaries,
        &graph_data,
        limits.mz.value,
        &unique_peptide_lookup,
    );
    write!(output, "</div></div>").unwrap();
    // General stats
    general_stats(
        &mut output,
        spectrum,
        fragments,
        multiple_peptidoforms,
        multiple_peptides,
        model,
        mass_mode,
    );

    //write!(output, "</div>").unwrap();
    (output, limits)
}

type Boundaries = (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64);
type SpectrumGraphData = Vec<Point>;
struct Point {
    annotation: Vec<(Fragment, Vec<MatchedIsotopeDistribution>)>,
    assigned: Option<RelAndAbs<f64>>,
    unassigned: UnassignedData,
    mz: MassOverCharge,
    mass: Mass,
    intensity: f64,
}

struct UnassignedData {
    a: Option<RelAndAbs<(f64, Fragment)>>,
    b: Option<RelAndAbs<(f64, Fragment)>>,
    c: Option<RelAndAbs<(f64, Fragment)>>,
    x: Option<RelAndAbs<(f64, Fragment)>>,
    y: Option<RelAndAbs<(f64, Fragment)>>,
    z: Option<RelAndAbs<(f64, Fragment)>>,
}

#[derive(Default)]
struct RelAndAbs<T> {
    rel: T,
    abs: T,
}

impl<T: Clone> RelAndAbs<T> {
    fn min_by(a: T, b: T, cmp: &impl Fn(T, T) -> Ordering) -> T {
        match cmp(a.clone(), b.clone()) {
            Ordering::Equal => a,
            Ordering::Greater => a,
            Ordering::Less => b,
        }
    }
}

impl<A, B: Clone> RelAndAbs<(A, &B)> {
    fn into_owned(self) -> RelAndAbs<(A, B)> {
        RelAndAbs {
            rel: (self.rel.0, self.rel.1.clone()),
            abs: (self.abs.0, self.abs.1.clone()),
        }
    }
}

impl<'a> RelAndAbs<(f64, &'a Fragment)> {
    fn fold(&mut self, fragment: &'a Fragment, point: &AnnotatedPeak, mass_mode: MassMode) {
        self.rel = Self::min_by(
            self.rel,
            (
                ((point.experimental_mz.value - fragment.mz(mass_mode).value)
                    / fragment.mz(mass_mode).value
                    * 1e6),
                fragment,
            ),
            &|a: (f64, &Fragment), b: (f64, &Fragment)| b.0.abs().total_cmp(&a.0.abs()),
        );
        self.abs = Self::min_by(
            self.abs,
            (
                (point.experimental_mz - fragment.mz(mass_mode)).value,
                fragment,
            ),
            &|a: (f64, &Fragment), b: (f64, &Fragment)| b.0.abs().total_cmp(&a.0.abs()),
        )
    }
}

fn get_data(data: &Option<RelAndAbs<(f64, Fragment)>>, ion: char) -> [(String, String); 4] {
    [
        (
            format!("u-{ion}-rel-value"),
            data.as_ref()
                .map_or("undefined".to_string(), |v| v.rel.0.to_string()),
        ),
        (
            format!("u-{ion}-rel-fragment"),
            data.as_ref()
                .map_or("undefined".to_string(), |v| v.rel.1.to_string()),
        ),
        (
            format!("u-{ion}-abs-value"),
            data.as_ref()
                .map_or("undefined".to_string(), |v| v.abs.0.to_string()),
        ),
        (
            format!("u-{ion}-abs-fragment"),
            data.as_ref()
                .map_or("undefined".to_string(), |v| v.abs.1.to_string()),
        ),
    ]
}

fn get_unassigned_data(
    point: &AnnotatedPeak,
    fragments: &[Fragment],
    model: &Model,
    mass_mode: MassMode,
) -> UnassignedData {
    let mut a = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    let mut b = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    let mut c = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    let mut x = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    let mut y = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    let mut z = RelAndAbs {
        rel: (f64::MAX, &Fragment::default()),
        abs: (f64::MAX, &Fragment::default()),
    };
    for fragment in fragments {
        match &fragment.ion {
            FragmentType::a(_) => a.fold(fragment, point, mass_mode),
            FragmentType::b(_) => b.fold(fragment, point, mass_mode),
            FragmentType::c(_) => c.fold(fragment, point, mass_mode),
            FragmentType::x(_) => x.fold(fragment, point, mass_mode),
            FragmentType::y(_) => y.fold(fragment, point, mass_mode),
            FragmentType::z(_) => z.fold(fragment, point, mass_mode),
            _ => (),
        }
    }
    UnassignedData {
        a: (model.a.0 != Location::None).then_some(a.into_owned()),
        b: (model.b.0 != Location::None).then_some(b.into_owned()),
        c: (model.c.0 != Location::None).then_some(c.into_owned()),
        x: (model.x.0 != Location::None).then_some(x.into_owned()),
        y: (model.y.0 != Location::None).then_some(y.into_owned()),
        z: (model.z.0 != Location::None).then_some(z.into_owned()),
    }
}

fn spectrum_graph_boundaries(
    spectrum: &AnnotatedSpectrum,
    fragments: &[Fragment],
    model: &Model,
    mass_mode: MassMode,
) -> (SpectrumGraphData, Boundaries) {
    let data: SpectrumGraphData = spectrum
        .spectrum()
        .map(|point| Point {
            annotation: point.annotation.clone(),
            assigned: (!point.annotation.is_empty()).then(|| RelAndAbs {
                rel: point
                    .annotation
                    .iter()
                    .map(|(f, _)| {
                        (f.mz(mass_mode).value - point.experimental_mz.value)
                            / f.mz(mass_mode).value
                            * 1e6
                    })
                    .min_by(|a, b| b.abs().total_cmp(&a.abs()))
                    .unwrap(),
                abs: point
                    .annotation
                    .iter()
                    .map(|(f, _)| (f.mz(mass_mode) - point.experimental_mz).value)
                    .min_by(|a, b| b.abs().total_cmp(&a.abs()))
                    .unwrap(),
            }),
            unassigned: get_unassigned_data(point, fragments, model, mass_mode),
            mz: point.experimental_mz,
            mass: da(point.experimental_mz.value * point.charge.value as f64),
            intensity: point.intensity.0,
        })
        .collect();
    let bounds = data.iter().fold(
        (
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
        ),
        |acc, point| {
            (
                acc.0
                    .max(point.assigned.as_ref().map_or(f64::MIN, |r| r.rel)), // rel
                acc.1
                    .min(point.assigned.as_ref().map_or(f64::MIN, |r| r.rel)),
                acc.2
                    .max(point.assigned.as_ref().map_or(f64::MIN, |r| r.abs)), // abs
                acc.3
                    .min(point.assigned.as_ref().map_or(f64::MIN, |r| r.abs)),
                acc.4.max(point.mz.value), // mz
                acc.5.min(point.mz.value),
                acc.6.max(point.mass.value), // mass
                acc.7.min(point.mass.value),
                acc.8.max(point.intensity), // intensity
                acc.9.min(point.intensity),
            )
        },
    );
    (data, bounds)
}

fn render_error_graph(
    output: &mut String,
    boundaries: &Boundaries,
    data: &SpectrumGraphData,
    x_max: f64,
    unique_peptide_lookup: &[(usize, usize)],
) {
    write!(output, "<div class='error-graph-y-axis'>").unwrap();
    write!(output, "<span class='max'>{:.2}</span>", boundaries.2).unwrap();
    write!(
        output,
        "<span class='title' id='error-graph-y-title'>Y title</span>"
    )
    .unwrap();
    write!(output, "<span class='min'>{:.2}</span>", boundaries.3).unwrap();
    write!(output, "<div id='error-graph-density'></div></div>").unwrap();
    write!(output, "<div id='error-graph' class='error-graph canvas'>",).unwrap();
    write!(output, "<div class='x-axis'>").unwrap();
    write!(output, "<span class='min'>0</span>").unwrap();
    write!(output, "<span class='max'>{:.2}</span>", x_max).unwrap();
    write!(output, "</div>").unwrap();
    write!(
        output,
        "<span class='ruler'><span id='ruler-value'>XX</span></span>"
    )
    .unwrap();

    for point in data {
        write!(
            output,
            "{}",
            HtmlTag::span
                .new()
                .header(
                    "class",
                    format!(
                        "point {}",
                        get_classes(&point.annotation, unique_peptide_lookup)
                    )
                )
                .style(format!(
                    "--mz:{};--intensity:{};",
                    point.mz.value, point.intensity
                ))
                .data(
                    [
                        (
                            "a-rel",
                            point
                                .assigned
                                .as_ref()
                                .map_or("undefined".to_string(), |r| r.rel.to_string())
                        ),
                        (
                            "a-abs",
                            point
                                .assigned
                                .as_ref()
                                .map_or("undefined".to_string(), |r| r.abs.to_string())
                        ),
                        ("intensity", point.intensity.to_string()),
                        ("mz", point.mz.value.to_string()),
                    ]
                    .into_iter()
                    .map(|(a, b)| (a.to_string(), b))
                    .chain(get_data(&point.unassigned.a, 'a'))
                    .chain(get_data(&point.unassigned.b, 'b'))
                    .chain(get_data(&point.unassigned.c, 'c'))
                    .chain(get_data(&point.unassigned.x, 'x'))
                    .chain(get_data(&point.unassigned.y, 'y'))
                    .chain(get_data(&point.unassigned.z, 'z'))
                )
        )
        .unwrap();
    }
    write!(output, "</div>").unwrap();
}

/// Get all applicable classes for a set of annotations.
/// These are:
///   * The ion type(s) (eg 'precursor', 'y') (and 'multi' if there are multiple)
///   * The peptidoform(s) (eg 'p0', 'p1') (and 'mp' if there are multiple peptides)
///   * The peptide(s) (eg 'p0-2', 'p2-1') (and 'mpp' if there are multiple peptides)
///   * The position(s) (eg 'p0-2-3', 'p2-1-123')
///   * The unique peptide index (eg 'pu1', 'pu12')
fn get_classes(
    annotations: &[(Fragment, Vec<MatchedIsotopeDistribution>)],
    unique_peptide_lookup: &[(usize, usize)],
) -> String {
    let mut output = Vec::new();
    let mut shared_ion = annotations.first().map(|a| a.0.ion.kind());
    let mut first_peptidoform_index = None;
    let mut first_peptide_index = None;
    for (annotation, _) in annotations {
        output.push(annotation.ion.label().to_string());
        output.push(format!("p{}", annotation.peptidoform_index));
        output.push(format!(
            "p{}-{}",
            annotation.peptidoform_index, annotation.peptide_index
        ));
        if let Some(num) = first_peptidoform_index {
            if num != annotation.peptidoform_index {
                if !output.contains(&"mp".to_string()) {
                    output.push("mp".to_string());
                }
            }
        } else {
            first_peptidoform_index = Some(annotation.peptidoform_index);
        }
        if let (Some(first_peptidoform_index), Some(fist_peptide_index)) =
            (first_peptidoform_index, first_peptide_index)
        {
            if first_peptidoform_index != annotation.peptidoform_index
                || fist_peptide_index != annotation.peptide_index
            {
                if !output.contains(&"mpp".to_string()) {
                    output.push("mpp".to_string())
                };
                let pu = format!(
                    "pu{}",
                    unique_peptide_lookup
                        .iter()
                        .position(
                            |id| *id == (annotation.peptidoform_index, annotation.peptide_index)
                        )
                        .unwrap()
                );
                if !output.contains(&pu) {
                    output.push(pu)
                };
            }
        } else {
            first_peptide_index = Some(annotation.peptide_index);
            output.push(format!(
                "pu{}",
                unique_peptide_lookup
                    .iter()
                    .position(|id| *id == (annotation.peptidoform_index, annotation.peptide_index))
                    .unwrap()
            ))
        }
        if let Some(pos) = annotation.ion.position() {
            output.push(format!(
                "p{}-{}-{}",
                annotation.peptidoform_index, annotation.peptide_index, pos.sequence_index
            ));
        }
        if annotation.ion.kind() == FragmentKind::Oxonium {
            output.push("oxonium".to_string());
        }
        if let Some(ion) = &shared_ion {
            if *ion != annotation.ion.kind() {
                shared_ion = None;
            }
        }
    }
    if shared_ion.is_none() {
        output.push("multi".to_string())
    }
    output = output.into_iter().unique().collect();
    if annotations.is_empty() {
        "unassigned".to_string()
    } else {
        output.join(" ")
    }
}

type PositionCoverage = Vec<Vec<Vec<HashSet<FragmentType>>>>;

pub struct Limits {
    pub mz: MassOverCharge,
    pub intensity: f64,
    pub intensity_unassigned: f64,
}

fn get_overview(spectrum: &AnnotatedSpectrum) -> (Limits, PositionCoverage) {
    let mut output: PositionCoverage = spectrum
        .peptide
        .peptidoforms()
        .iter()
        .map(|p| {
            p.peptides()
                .iter()
                .map(|p| vec![HashSet::new(); p.sequence.len()])
                .collect()
        })
        .collect();
    let mut max_mz: MassOverCharge = MassOverCharge::new::<mz>(0.0);
    let mut max_intensity: f64 = 0.0;
    let mut max_intensity_unassigned: f64 = 0.0;
    for peak in spectrum.spectrum() {
        max_mz = max_mz.max(peak.experimental_mz);
        max_intensity_unassigned = max_intensity_unassigned.max(peak.intensity.0);
        if !peak.annotation.is_empty() {
            max_intensity = max_intensity.max(peak.intensity.0);
            peak.annotation.iter().for_each(|(fragment, _)| {
                fragment.ion.position().map(|i| {
                    output[fragment.peptidoform_index][fragment.peptide_index][i.sequence_index]
                        .insert(fragment.ion.clone())
                });
            });
        }
    }
    (
        Limits {
            mz: max_mz * 1.01,
            intensity: max_intensity * 1.01,
            intensity_unassigned: max_intensity_unassigned * 1.01,
        },
        output,
    )
}

fn render_peptide(
    output: &mut String,
    spectrum: &AnnotatedSpectrum,
    overview: PositionCoverage,
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    unique_peptide_lookup: &mut Vec<(usize, usize)>,
) {
    write!(output, "<div class='complex-peptide'>").unwrap();
    let mut cross_link_lookup = Vec::new();
    for (peptidoform_index, peptidoform) in spectrum.peptide.peptidoforms().iter().enumerate() {
        for (peptide_index, peptide) in peptidoform.peptides().iter().enumerate() {
            render_linear_peptide(
                output,
                peptide,
                &overview[peptidoform_index][peptide_index],
                peptidoform_index,
                peptide_index,
                unique_peptide_lookup.len(),
                multiple_peptidoforms,
                multiple_peptides,
                &mut cross_link_lookup,
            );
            unique_peptide_lookup.push((peptidoform_index, peptide_index));
        }
    }
    write!(output, "</div>").unwrap();
}

fn render_linear_peptide(
    output: &mut String,
    peptide: &LinearPeptide<Linked>,
    overview: &[HashSet<FragmentType>],
    peptidoform_index: usize,
    peptide_index: usize,
    unique_peptide_index: usize,
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    cross_link_lookup: &mut Vec<CrossLinkName>,
) {
    write!(
        output,
        "<div class='peptide pu{unique_peptide_index} p{peptidoform_index}'>"
    )
    .unwrap();
    if multiple_peptides || multiple_peptidoforms {
        write!(
            output,
            "{}",
            HtmlTag::span
                .new()
                .class("name")
                .data([("pos", format!("{peptidoform_index}-{peptide_index}"))])
                .content(if multiple_peptidoforms && multiple_peptides {
                    format!("{}.{}", peptidoform_index + 1, peptide_index + 1)
                } else if multiple_peptidoforms {
                    (peptidoform_index + 1).to_string()
                } else if multiple_peptides {
                    (peptide_index + 1).to_string()
                } else {
                    String::new()
                })
        )
        .unwrap();
    }
    if peptide.n_term.is_some() {
        write!(output, "<span class='modification term'></span>").unwrap();
    }
    for (index, (pos, ions)) in peptide.sequence.iter().zip(overview).enumerate() {
        let mut classes = String::new();
        let mut xl_indices = Vec::new();
        let mut xl_names = Vec::new();
        let mut xl_peptides = Vec::new();
        for m in &pos.modifications {
            if let Modification::CrossLink { peptide, name, .. } = m {
                let xl_index = cross_link_lookup
                    .iter()
                    .position(|xl| *xl == *name)
                    .unwrap_or_else(|| {
                        cross_link_lookup.push(name.clone());
                        cross_link_lookup.len() - 1
                    });
                write!(classes, " c{xl_index}").unwrap();
                xl_indices.push(xl_index + 1);
                xl_names.push(name);
                if *peptide != peptide_index {
                    xl_peptides.push(*peptide + 1);
                }
            }
        }

        let cross_links = format!(
            "xl.{}{}",
            xl_indices.iter().join(","),
            if xl_peptides.is_empty() {
                String::new()
            } else {
                format!("p{}", xl_peptides.iter().join(","))
            }
        );

        let cross_links_compact = format!("x{}", xl_indices.iter().join(","),);

        if pos
            .modifications
            .iter()
            .any(|m| matches!(m, Modification::Simple(_)))
        {
            write!(classes, " modification").unwrap();
        }
        if !xl_indices.is_empty() {
            write!(classes, " cross-link").unwrap();
        }
        if !pos.possible_modifications.is_empty() {
            write!(classes, " possible-modification").unwrap();
        }
        if !classes.is_empty() {
            classes = format!(" class='{}'", classes.trim());
        }
        write!(
            output,
            "<span data-pos='{peptidoform_index}-{peptide_index}-{index}' data-cross-links='{cross_links}' data-cross-links-compact='{cross_links_compact}'{classes} tabindex='0' title='N terminal position: {}, C terminal position: {}{}'>{}",
            index + 1,
            peptide.sequence.len() - index,
            if xl_names.is_empty() {
                String::new()
            } else {
                format!(", Cross-link{}: {}", if xl_names.len() == 1{""} else {"s"}, xl_names.iter().join(", "))
            },
            pos.aminoacid.char(),
        )
        .unwrap();
        for ion in ions {
            if !matches!(
                ion,
                FragmentType::immonium(_, _) | FragmentType::m(_, _) | FragmentType::diagnostic(_)
            ) {
                write!(
                    output,
                    "<span class='corner {}'></span>",
                    ion.label().trim_end_matches('·')
                )
                .unwrap();
            }
        }
        write!(output, "</span>").unwrap();
    }
    if peptide.c_term.is_some() {
        write!(output, "<span class='modification term'></span>").unwrap();
    }
    if let Some(charge_carriers) = &peptide.charge_carriers {
        write!(
            output,
            "<span class='charge-carriers'>/{charge_carriers}</span>",
        )
        .unwrap();
    }
    write!(output, "</div>").unwrap();
}

fn render_spectrum(
    output: &mut String,
    spectrum: &AnnotatedSpectrum,
    fragments: &[Fragment],
    boundaries: &Boundaries,
    limits: &Limits,
    selection: &str,
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    multiple_glycans: bool,
    mass_mode: MassMode,
    unique_peptide_lookup: &[(usize, usize)],
) {
    write!(
        output,
        "<div class='canvas-wrapper label' aria-hidden='true' style='--min-mz:0;--max-mz:{0};--max-intensity:{1};--y-max:{2};--y-min:{3};--abs-max-initial:{2};--abs-min-initial:{3};--rel-max-initial:{4};--rel-min-initial:{5};' data-initial-max-mz='{0}' data-initial-max-intensity='{1}' data-initial-max-intensity-assigned='{6}'>",
        limits.mz.value, limits.intensity_unassigned, boundaries.2, boundaries.3, boundaries.0, boundaries.1, limits.intensity
    )
    .unwrap();
    write!(
        output,
        "<div class='y-axis'><span class='tick'><span class='n0'>0</span></span>"
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        limits.intensity_unassigned / 4.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        limits.intensity_unassigned / 2.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        3.0 * limits.intensity_unassigned / 4.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span class='last'>{:.2}</span></span>",
        limits.intensity_unassigned
    )
    .unwrap();
    write!(output, "</div>").unwrap();
    write!(output, "<div class='canvas canvas-spectrum'>").unwrap();
    write!(
        output,
        "<span class='selection {selection}' hidden='true'></span>"
    )
    .unwrap();

    for peak in spectrum.spectrum() {
        write!(
            output,
            "<span class='peak {}' style='--mz:{};--intensity:{};' data-label='{}' {}>{}</span>",
            get_classes(&peak.annotation, unique_peptide_lookup),
            peak.experimental_mz.value,
            peak.intensity,
            (peak.experimental_mz.value * 100.0).round() / 100.0,
            if peak.intensity.0 / limits.intensity >= 0.1 {
                "data-show-label='true'"
            } else {
                ""
            },
            get_label(
                &peak.annotation,
                multiple_peptidoforms,
                multiple_peptides,
                multiple_glycans
            ),
        )
        .unwrap();
    }
    for peak in fragments {
        write!(
            output,
            "<span class='theoretical peak {}' style='--mz:{};' data-label='{}'>{}</span>",
            get_classes(&[(peak.clone(), Vec::new())], unique_peptide_lookup),
            peak.mz(mass_mode).value,
            (peak.mz(mass_mode).value * 10.0).round() / 10.0,
            get_label(
                &[(peak.clone(), Vec::new())],
                multiple_peptidoforms,
                multiple_peptides,
                multiple_glycans
            ),
        )
        .unwrap();
    }
    write!(output, "</div>").unwrap();
    write!(
        output,
        "<div class='x-axis'><span class='tick'><span class='n0'>0</span></span>"
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        limits.mz.value / 4.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        limits.mz.value / 2.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span>{:.2}</span></span>",
        3.0 * limits.mz.value / 4.0
    )
    .unwrap();
    write!(
        output,
        "<span class='tick'><span class='last'>{:.2}</span></span>",
        limits.mz.value
    )
    .unwrap();
    write!(output, "</div>").unwrap();
}

pub fn spectrum_table(
    spectrum: &AnnotatedSpectrum,
    fragments: &[Fragment],
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
) -> String {
    fn generate_text(
        annotation: &(Fragment, Vec<MatchedIsotopeDistribution>),
    ) -> (String, String, String) {
        if let Some(pos) = annotation.0.ion.position() {
            (
                (pos.sequence_index + 1).to_string(),
                pos.series_number.to_string(),
                annotation.0.ion.label().to_string(),
            )
        } else if let Some(pos) = annotation.0.ion.glycan_position() {
            (
                pos.attachment(),
                format!("{}{}", pos.series_number, pos.branch_names()),
                annotation.0.ion.label().to_string(),
            )
        } else if let FragmentType::Oxonium(breakages) = &annotation.0.ion {
            (
                breakages
                    .first()
                    .map(|b| b.position().attachment())
                    .unwrap_or("-".to_string()),
                breakages
                    .iter()
                    .filter(|b| !matches!(b, GlycanBreakPos::End(_)))
                    .map(|b| format!("{}<sub>{}</sub>", b.label(), b.position().label()))
                    .join(""),
                "oxonium".to_string(),
            )
        } else if let FragmentType::Y(bonds) = &annotation.0.ion {
            (
                bonds
                    .first()
                    .map(|b| b.attachment())
                    .unwrap_or("-".to_string()),
                bonds.iter().map(|b| b.label()).join(""),
                "Y".to_string(),
            )
        } else if let FragmentType::immonium(pos, aa) = &annotation.0.ion {
            (
                (pos.sequence_index + 1).to_string(),
                pos.series_number.to_string(),
                format!("immonium {}", aa.char()),
            )
        } else {
            // precursor
            (
                "-".to_string(),
                "-".to_string(),
                annotation.0.ion.label().to_string(),
            )
        }
    }
    let mut output = String::new();
    write!(
        output,
        "<p id='export-sequence' title='The inputted peptide written as fully compatible with the ProForma spec. So removes things like custom modifications.'>Universal ProForma definition: <span>{}</span></p>
        <label class='show-unassigned'><input type='checkbox' switch/>Show background peaks</label>
        <label class='show-matched'><input type='checkbox' switch checked/>Show annotated peaks</label>
        <label class='show-missing-fragments'><input type='checkbox' switch/>Show missing fragments</label>
        <table id='spectrum-table' class='wide-table'>
            <thead><tr>
                {}
                {}
                <th>Position</th>
                <th>Ion type</th>
                <th>Loss</th>
                <th>Intensity</th>
                <th>mz Theoretical</th>
                <th>Formula</th>
                <th>mz Error (Th)</th>
                <th>mz Error (ppm)</th>
                <th>Charge</th>
                <th>Series Number</th>
                <th>Additional label</th>
            </tr></thead><tdata>",
        spectrum.peptide,
        if multiple_peptidoforms {
            "<th>Peptidoform</th>"
        } else {
            ""
        },
        if multiple_peptides {
            "<th>Peptide</th>"
        } else {
            ""
        }
    )
    .unwrap();
    // class followed by all data
    let mut data = Vec::new();
    for peak in spectrum.spectrum() {
        if peak.annotation.is_empty() {
            data.push((
                peak.experimental_mz.value,
                [
                    "unassigned".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    format!("{:.2}", peak.intensity),
                    format!("{:.2}", peak.experimental_mz.value),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                ],
            ));
        } else {
            for full @ (annotation, _) in &peak.annotation {
                let (sequence_index, series_number, label) = generate_text(full);
                data.push((
                    peak.experimental_mz.value,
                    [
                        "matched".to_string(),
                        if multiple_peptidoforms {
                            format!("{}", annotation.peptidoform_index + 1)
                        } else {
                            String::new()
                        },
                        if multiple_peptides {
                            format!("{}", annotation.peptide_index + 1)
                        } else {
                            String::new()
                        },
                        sequence_index.to_string(),
                        label.to_string(),
                        annotation
                            .neutral_loss
                            .as_ref()
                            .map_or(String::new(), display_neutral_loss),
                        format!("{:.2}", peak.intensity),
                        format!("{:.2}", peak.experimental_mz.value),
                        display_formula(&annotation.formula, true),
                        format!(
                            "{:.5}",
                            (annotation.mz(MassMode::Monoisotopic) - peak.experimental_mz)
                                .abs()
                                .value
                        ),
                        format!(
                            "{:.2}",
                            ((annotation.mz(MassMode::Monoisotopic) - peak.experimental_mz).abs()
                                / annotation.mz(MassMode::Monoisotopic)
                                * 1e6)
                                .value
                        ),
                        format!("{:+}", annotation.charge.value),
                        series_number,
                        String::new(), //format!("{:?}", annotation.formula.labels()),
                    ],
                ));
            }
        }
    }
    for fragment in fragments {
        if !spectrum
            .spectrum()
            .any(|p| p.annotation.iter().any(|a| a.0 == *fragment))
        {
            let (sequence_index, series_number, label) =
                generate_text(&(fragment.clone(), Vec::new()));
            data.push((
                fragment.mz(MassMode::Monoisotopic).value,
                [
                    "fragment".to_string(),
                    if multiple_peptidoforms {
                        format!("{}", fragment.peptidoform_index + 1)
                    } else {
                        String::new()
                    },
                    if multiple_peptides {
                        format!("{}", fragment.peptide_index + 1)
                    } else {
                        String::new()
                    },
                    sequence_index.to_string(),
                    label.to_string(),
                    fragment
                        .neutral_loss
                        .as_ref()
                        .map_or(String::new(), display_neutral_loss),
                    "-".to_string(),
                    format!("{:.2}", fragment.mz(MassMode::Monoisotopic).value),
                    display_formula(&fragment.formula, true),
                    "-".to_string(),
                    "-".to_string(),
                    format!("{:+}", fragment.charge.value),
                    series_number,
                    String::new(), //format!("{:?}", fragment.formula.labels()),
                ],
            ))
        }
    }
    data.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    for row in data {
        write!(output, "<tr class='{}'>", row.1[0]).unwrap();
        for cell in &row.1[if multiple_peptides { 1 } else { 2 }..] {
            write!(output, "<td>{}</td>", cell).unwrap();
        }
        write!(output, "</tr>").unwrap();
    }
    write!(output, "</tdata></table>").unwrap();
    output
}

fn general_stats(
    output: &mut String,
    spectrum: &AnnotatedSpectrum,
    fragments: &[Fragment],
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    model: &Model,
    mass_mode: MassMode,
) {
    fn format(recovered: Recovered<u32>) -> String {
        format!(
            "{:.2}% ({}/{})",
            recovered.fraction() * 100.0,
            recovered.found,
            recovered.total
        )
    }
    fn format_f64(recovered: Recovered<f64>) -> String {
        format!(
            "{:.2}% ({:.2e}/{:.2e})",
            recovered.fraction() * 100.0,
            recovered.found,
            recovered.total
        )
    }
    fn format_fdr(fdr: &Fdr) -> String {
        format!("{:.2}% ({:.3} σ)", fdr.fdr() * 100.0, fdr.sigma())
    }

    let mut mass_row = String::new();
    let mut fragments_row = String::new();
    let mut fragments_details_row = String::new();
    let mut peaks_row = String::new();
    let mut peaks_details_row = String::new();
    let mut intensity_row = String::new();
    let mut intensity_details_row = String::new();
    let mut positions_row = String::new();
    let mut positions_details_row = String::new();
    let mut fdr_row = String::new();

    let (combined_scores, peptide_scores) = spectrum.scores(fragments);
    let (combined_fdr, peptide_fdr) = spectrum.fdr(fragments, model, mass_mode);

    for (peptidoform_index, score) in peptide_scores.iter().enumerate() {
        for (peptide_index, score) in score.iter().enumerate() {
            let precursor = spectrum.peptide.peptidoforms()[peptidoform_index].peptides()
                [peptide_index]
                .clone()
                .linear()
                .map_or("Part of peptidoform".to_string(), |p| {
                    p.formulas().iter().map(display_masses).join(", ")
                });
            write!(mass_row, "<td>{precursor}</td>").unwrap();
            match score.score {
                Score::Position {
                    fragments,
                    peaks,
                    intensity,
                    positions,
                } => {
                    write!(fragments_row, "<td>{}</td>", format(fragments)).unwrap();
                    write!(peaks_row, "<td>{}</td>", format(peaks)).unwrap();
                    write!(intensity_row, "<td>{}</td>", format_f64(intensity)).unwrap();
                    write!(positions_row, "<td>{} (positions)</td>", format(positions)).unwrap();
                    write!(
                        fdr_row,
                        "<td>{}</td>",
                        format_fdr(&peptide_fdr[peptidoform_index][peptide_index])
                    )
                    .unwrap();
                }
                Score::UniqueFormulas {
                    fragments,
                    peaks,
                    intensity,
                    unique_formulas,
                } => {
                    write!(fragments_row, "<td>{}</td>", format(fragments)).unwrap();
                    write!(peaks_row, "<td>{}</td>", format(peaks)).unwrap();
                    write!(intensity_row, "<td>{}</td>", format_f64(intensity)).unwrap();
                    write!(
                        positions_row,
                        "<td>{} (unique compositions)</td>",
                        format(unique_formulas)
                    )
                    .unwrap();
                    write!(
                        fdr_row,
                        "<td>{}</td>",
                        format_fdr(&peptide_fdr[peptidoform_index][peptide_index])
                    )
                    .unwrap();
                }
            }
            write!(fragments_details_row, "<td><table>").unwrap();
            write!(peaks_details_row, "<td><table>").unwrap();
            write!(intensity_details_row, "<td><table>").unwrap();
            write!(positions_details_row, "<td><table>").unwrap();
            for (ion, score) in &score.ions {
                match score {
                    Score::Position {
                        fragments,
                        peaks,
                        intensity,
                        positions,
                    } => {
                        write!(
                            fragments_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format(*fragments)
                        )
                        .unwrap();
                        write!(
                            peaks_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format(*peaks)
                        )
                        .unwrap();
                        write!(
                            intensity_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format_f64(*intensity)
                        )
                        .unwrap();
                        write!(
                            positions_details_row,
                            "<tr><td>{ion}</td><td>{} (positions)</td></tr>",
                            format(*positions)
                        )
                        .unwrap();
                    }
                    Score::UniqueFormulas {
                        fragments,
                        peaks,
                        intensity,
                        unique_formulas,
                    } => {
                        write!(
                            fragments_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format(*fragments)
                        )
                        .unwrap();
                        write!(
                            peaks_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format(*peaks)
                        )
                        .unwrap();
                        write!(
                            intensity_details_row,
                            "<tr><td>{ion}</td><td>{}</td></tr>",
                            format_f64(*intensity)
                        )
                        .unwrap();
                        write!(
                            positions_details_row,
                            "<tr><td>{ion}</td><td>{} (unique compositions)</td></tr>",
                            format(*unique_formulas)
                        )
                        .unwrap();
                    }
                }
            }
            write!(fragments_details_row, "</table></td>").unwrap();
            write!(peaks_details_row, "</table></td>").unwrap();
            write!(intensity_details_row, "</table></td>").unwrap();
            write!(positions_details_row, "</table></td>").unwrap();
        }
    }

    write!(output, "<label><input type='checkbox' switch id='general-stats-show-details'>Show detailed fragment breakdown</label><table class='general-stats'>").unwrap();
    if multiple_peptidoforms || multiple_peptides {
        write!(output, "<tr><td>General Statistics</td>").unwrap();
        for (peptidoform_index, peptidoform) in spectrum.peptide.peptidoforms().iter().enumerate() {
            for peptide_index in 0..peptidoform.peptides().len() {
                if multiple_peptidoforms && multiple_peptides {
                    write!(
                        output,
                        "<td>Peptidoform {} Peptide {}</td>",
                        peptidoform_index + 1,
                        peptide_index + 1
                    )
                    .unwrap();
                } else if multiple_peptidoforms {
                    write!(output, "<td>Peptidoform {} </td>", peptidoform_index + 1,).unwrap();
                } else if multiple_peptides {
                    write!(output, "<td>Peptide {}</td>", peptide_index + 1).unwrap();
                }
            }
        }
        write!(output, "<td>Combined</td></tr>").unwrap();
        // Add a combined stats column
        write!(mass_row, "<td>-</td>").unwrap();
        match combined_scores.score {
            Score::Position {
                fragments,
                peaks,
                intensity,
                positions,
            } => {
                write!(fragments_row, "<td>{}</td>", format(fragments)).unwrap();
                write!(peaks_row, "<td>{}</td>", format(peaks)).unwrap();
                write!(intensity_row, "<td>{}</td>", format_f64(intensity)).unwrap();
                write!(positions_row, "<td>{} (positions)</td>", format(positions)).unwrap();
                write!(fdr_row, "<td>{}</td>", format_fdr(&combined_fdr)).unwrap();
            }
            Score::UniqueFormulas {
                fragments,
                peaks,
                intensity,
                unique_formulas,
            } => {
                write!(fragments_row, "<td>{}</td>", format(fragments)).unwrap();
                write!(peaks_row, "<td>{}</td>", format(peaks)).unwrap();
                write!(intensity_row, "<td>{}</td>", format_f64(intensity)).unwrap();
                write!(
                    positions_row,
                    "<td>{} (unique compositions)</td>",
                    format(unique_formulas)
                )
                .unwrap();
                write!(fdr_row, "<td>{}</td>", format_fdr(&combined_fdr)).unwrap();
            }
        }
        write!(fragments_details_row, "<td><table>").unwrap();
        write!(peaks_details_row, "<td><table>").unwrap();
        write!(intensity_details_row, "<td><table>").unwrap();
        write!(positions_details_row, "<td><table>").unwrap();
        for (ion, score) in combined_scores.ions {
            match score {
                Score::Position {
                    fragments,
                    peaks,
                    intensity,
                    positions,
                } => {
                    write!(
                        fragments_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format(fragments)
                    )
                    .unwrap();
                    write!(
                        peaks_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format(peaks)
                    )
                    .unwrap();
                    write!(
                        intensity_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format_f64(intensity)
                    )
                    .unwrap();
                    write!(
                        positions_details_row,
                        "<tr><td>{ion}</td><td>{} (positions)</td></tr>",
                        format(positions)
                    )
                    .unwrap();
                }
                Score::UniqueFormulas {
                    fragments,
                    peaks,
                    intensity,
                    unique_formulas,
                } => {
                    write!(
                        fragments_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format(fragments)
                    )
                    .unwrap();
                    write!(
                        peaks_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format(peaks)
                    )
                    .unwrap();
                    write!(
                        intensity_details_row,
                        "<tr><td>{ion}</td><td>{}</td></tr>",
                        format_f64(intensity)
                    )
                    .unwrap();
                    write!(
                        positions_details_row,
                        "<tr><td>{ion}</td><td>{} (unique compositions)</td></tr>",
                        format(unique_formulas)
                    )
                    .unwrap();
                }
            }
        }
        write!(fragments_details_row, "</table></td>").unwrap();
        write!(peaks_details_row, "</table></td>").unwrap();
        write!(intensity_details_row, "</table></td>").unwrap();
        write!(positions_details_row, "</table></td>").unwrap();
    }
    write!(
        output,
        "<tr><td>Precursor Mass (M)</td>{mass_row}</tr>
        <tr><td>Fragments found</td>{fragments_row}</tr>
        <tr class='fragments-detail'><td>Fragments detailed</td>{fragments_details_row}</tr>
        <tr><td>Peaks annotated</td>{peaks_row}</tr>
        <tr class='fragments-detail'><td>Peaks detailed</td>{peaks_details_row}</tr>
        <tr><td>Intensity annotated</td>{intensity_row}</tr>
        <tr class='fragments-detail'><td>Intensity detailed</td>{intensity_details_row}</tr>
        <tr><td>Sequence positions covered</td>{positions_row}</tr>
        <tr class='fragments-detail'><td>Positions detailed</td>{positions_details_row}</tr>
        <tr><td title='FDR estimation by permutation; Tests how many matches are found when the spectrum is shifted from -25 to +25 Da plus π (to have non integer offsets). The percentage is the number found for the actual matches divided by the average found number for the shifted spectra. The number between brackets denotes the number of standard deviations the actual matches is from the shifted matches.'>FDR</td>{fdr_row}</tr>
    </table>"
    )
    .unwrap();
}

fn density_estimation<const STEPS: usize>(mut data: Vec<f64>) -> ([f64; STEPS], f64, f64) {
    let mut densities = [0.0; STEPS];
    if data.is_empty() {
        return (densities, 0.0, 0.0);
    }
    data.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let data = data; // Freeze mutability
    let min_value = data.first().copied().unwrap_or(f64::MIN); // Cannot crash as the data is longer than 0
    let max_value = data.last().copied().unwrap_or(f64::MAX);
    let len = data.len() as f64;
    let half = len / 2.0;
    let first_half_sum: f64 = data.iter().take(half.floor() as usize).sum();
    let last_half_sum: f64 = data.iter().skip(half.floor() as usize).sum();
    let mean: f64 = (first_half_sum + last_half_sum) / len;
    let standard_deviation: f64 =
        (data.iter().map(|p| (mean - p).powi(2)).sum::<f64>() / len).sqrt();
    let iqr: f64 = last_half_sum / half - first_half_sum / half;
    let h = 0.25 * standard_deviation.min(iqr / 1.34) * len.powf(-0.2);

    let gaussian_kernel =
        |x: f64| 1.0 / (2.0 * std::f64::consts::PI).sqrt() * (-1.0 / 2.0 * x.powi(2)).exp();
    let kde = |x: f64| {
        1.0 / (len * h)
            * data
                .iter()
                .map(|i| gaussian_kernel((x - i) / h))
                .sum::<f64>()
    };

    for (i, density) in densities.iter_mut().enumerate() {
        *density = kde(min_value + (max_value - min_value) / (STEPS - 1) as f64 * i as f64);
    }

    (densities, min_value, max_value)
}

#[tauri::command]
pub async fn density_graph(data: Vec<f64>) -> Result<String, ()> {
    const STEPS: usize = 256; // Determines the precision of the density
    let (densities, min, max) = density_estimation::<STEPS>(data);
    let max_density = densities
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(f64::MAX);
    let mut path = String::new();
    for (i, density) in densities.iter().rev().enumerate() {
        write!(
            &mut path,
            "{}{} {}",
            if i != 0 { " L " } else { "" },
            (max_density - density) / max_density * 100.0,
            i,
        )
        .unwrap();
    }
    Ok(format!("<svg viewBox='-1 0 100 {}' style='--min:{min};--max:{max};' preserveAspectRatio='none'><g class='density'><path class='line' d='M {}'></path><path class='volume' d='M 100 0 L {} L {} 0 Z'></path></g></svg>",
        STEPS-1,
        path,
        path, STEPS -1
        ))
}

pub fn display_masses(value: &MolecularFormula) -> HtmlElement {
    HtmlTag::span
        .new()
        .children([
            display_mass(value.monoisotopic_mass(), Some(MassMode::Monoisotopic)).into(),
            HtmlContent::Text(" / ".to_string()),
            display_mass(value.average_weight(), Some(MassMode::Average)).into(),
            HtmlContent::Text(" / ".to_string()),
            display_mass(value.most_abundant_mass(), Some(MassMode::MostAbundant)).into(),
        ])
        .clone()
}

pub fn display_mass(value: Mass, kind: Option<MassMode>) -> HtmlElement {
    let (num, suf, full) = engineering_notation(value.value, 3);
    let suf = suf.map_or(String::new(), |suf| suf.to_string());
    let kind = match kind {
        Some(MassMode::Average) => "Average weight, average over all isotopes\n",
        Some(MassMode::Monoisotopic) => {
            "Monoisotopic mass, most abundant isotope for each separate element\n"
        }
        Some(MassMode::MostAbundant) => {
            "Most abundant mass, averagine model of isotope distribution most abundant isotope mass\n"
        }
        _ => "",
    };
    HtmlTag::span
        .new()
        .class("mass")
        .header(
            "title",
            if suf.is_empty() {
                format!("{kind}{} Da", value.value)
            } else {
                format!("{kind}{} {}Da\n{} Da", full, suf, value.value)
            },
        )
        .content(format!("{} {}Da", num, suf))
        .clone()
}

/// Display the given value in engineering notation eg `1000` -> `10 k`, with the given number of decimal points and returns the suffix separately.
/// A value of `0.0` will result in the lowest possible suffix `0.0 q`.
fn engineering_notation(value: f64, precision: usize) -> (String, Option<char>, f64) {
    const BIG_SUFFIXES: &[char] = &[' ', 'k', 'M', 'G', 'T', 'P', 'E', 'Z', 'Y', 'R', 'Q'];
    const SMALL_SUFFIXES: &[char] = &[' ', 'm', 'μ', 'n', 'p', 'f', 'a', 'z', 'y', 'r', 'q'];
    let base = if value == 0.0 {
        0
    } else {
        ((value.abs().log10() / 3.0).floor() as isize).clamp(
            -(SMALL_SUFFIXES.len() as isize - 1),
            BIG_SUFFIXES.len() as isize - 1,
        )
    };
    match base.cmp(&0) {
        Ordering::Less => (
            format!(
                "{:.precision$}",
                value * 10_usize.pow(base.unsigned_abs() as u32 * 3) as f64,
            ),
            Some(SMALL_SUFFIXES[base.unsigned_abs()]),
            value * 10_usize.pow(base.unsigned_abs() as u32 * 3) as f64,
        ),
        Ordering::Equal => (format!("{value:.precision$}"), None, value),
        Ordering::Greater => (
            format!(
                "{:.precision$}",
                value / 10_usize.pow(base as u32 * 3) as f64,
            ),
            Some(BIG_SUFFIXES[base as usize]),
            value / 10_usize.pow(base as u32 * 3) as f64,
        ),
    }
}

pub fn display_stubs(formula: &(MolecularFormula, MolecularFormula), formatted: bool) -> String {
    format!(
        "{}:{}",
        display_formula(&formula.0, formatted),
        display_formula(&formula.1, formatted)
    )
}

pub fn display_formula(formula: &MolecularFormula, formatted: bool) -> String {
    if formatted {
        if formula.is_empty() {
            "<span class='formula empty'>(empty)</span>".to_string()
        } else {
            format!(
                "<span class='formula'>{}</span>",
                formula.hill_notation_html()
            )
        }
    } else if formula.is_empty() {
        "(empty)".to_string()
    } else {
        formula.hill_notation()
    }
}

pub fn display_neutral_loss(formula: &NeutralLoss) -> String {
    if formula.is_empty() {
        "<span class='formula empty'>(empty)</span>".to_string()
    } else {
        format!(
            "<span class='formula'>{}</span>",
            formula.hill_notation_html()
        )
    }
}

pub fn display_placement_rule(rule: &PlacementRule, formatted: bool) -> String {
    if formatted {
        match rule {
            PlacementRule::AminoAcid(aa, pos) => format!(
                "<span class='aminoacid'>{}</span>@<span class='position'>{pos}</span>",
                aa.iter().join("")
            ),
            PlacementRule::Terminal(pos) => format!("<span class='position'>{pos}</span>"),
            PlacementRule::Anywhere => "<span class='position'>Anywhere</span>".to_string(),
            PlacementRule::PsiModification(index, pos) => {
                format!(
                    "{}@<span class='position'>{pos}</span>",
                    Ontology::Psimod
                        .find_id(*index, None)
                        .unwrap_or_else(|| panic!(
                            "Invalid PsiMod placement rule, non existing modification {index}"
                        ))
                )
            }
        }
    } else {
        match rule {
            PlacementRule::AminoAcid(aa, pos) => format!("{}@{pos}", aa.iter().join("")),
            PlacementRule::Terminal(pos) => pos.to_string(),
            PlacementRule::Anywhere => "Anywhere".to_string(),
            PlacementRule::PsiModification(index, pos) => {
                format!(
                    "{}@{pos}",
                    Ontology::Psimod
                        .find_id(*index, None)
                        .unwrap_or_else(|| panic!(
                            "Invalid PsiMod placement rule, non existing modification {index}"
                        ))
                )
            }
        }
    }
}

pub fn link_modification(ontology: Ontology, id: usize, name: &str) -> String {
    if ontology == Ontology::Gnome {
        format!("<a onclick='document.getElementById(\"search-modification\").value=\"{0}:{1}\";document.getElementById(\"search-modification-button\").click()'>{0}:{1}</a>", ontology.char(), name)
    } else {
        format!("<a onclick='document.getElementById(\"search-modification\").value=\"{0}:{1}\";document.getElementById(\"search-modification-button\").click()'>{0}:{1}</a>", ontology.name(), id)
    }
}