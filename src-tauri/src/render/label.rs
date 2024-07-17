use itertools::Itertools;
use rustyms::{
    fragment::{FragmentType, GlycanBreakPos, MatchedIsotopeDistribution},
    AmbiguousLabel, Fragment,
};
use std::fmt::Write;

pub fn get_label(
    annotations: &[(Fragment, Vec<MatchedIsotopeDistribution>)],
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    multiple_glycans: bool,
) -> String {
    if annotations.is_empty() {
        String::new()
    } else {
        let mut shared_charge = Some(annotations[0].0.charge);
        let mut shared_ion = Some(annotations[0].0.ion.label());
        let mut shared_pos = Some(annotations[0].0.ion.position_label());
        let mut shared_peptidoform = Some(annotations[0].0.peptidoform_index);
        let mut shared_peptide = Some(annotations[0].0.peptide_index);
        let mut shared_glycan = Some(
            annotations[0]
                .0
                .ion
                .glycan_position()
                .map(|g| g.attachment()),
        );
        let mut shared_loss = Some(annotations[0].0.neutral_loss.clone());
        let mut shared_xl = Some(get_xl(&annotations[0].0));

        for (a, _) in annotations {
            if let Some(charge) = shared_charge {
                if charge != a.charge {
                    shared_charge = None;
                }
            }
            if let Some(ion) = &shared_ion {
                if *ion != a.ion.label() {
                    shared_ion = None;
                }
            }
            if let Some(pos) = &shared_pos {
                if *pos != a.ion.position_label() {
                    shared_pos = None;
                }
            }
            if let Some(peptidoform) = shared_peptidoform {
                if peptidoform != a.peptidoform_index {
                    shared_peptidoform = None;
                }
            }
            if let Some(peptide) = shared_peptide {
                if peptide != a.peptide_index {
                    shared_peptide = None;
                }
            }
            if let Some(glycan) = &shared_glycan {
                if *glycan != a.ion.glycan_position().map(|g| g.attachment()) {
                    shared_glycan = None;
                }
            }
            if let Some(loss) = &shared_loss {
                if loss != &a.neutral_loss {
                    shared_loss = None;
                }
            }
            if let Some(xl) = &shared_xl {
                if *xl != get_xl(a) {
                    shared_xl = None;
                }
            }
        }

        if shared_charge.is_none()
            && shared_ion.is_none()
            && shared_pos.is_none()
            && shared_peptidoform.is_none()
            && shared_peptide.is_none()
            && shared_glycan.is_none()
            && shared_loss.is_none()
            && shared_xl.is_none()
        {
            "*".to_string()
        } else {
            let charge_str = shared_charge
                .map(|charge| format!("{:+}", charge.value))
                .unwrap_or("*".to_string());
            let ion_str = shared_ion
                .map(|c| c.into_owned())
                .unwrap_or("*".to_string());
            let pos_str = shared_pos
                .map(|pos| pos.unwrap_or_default())
                .unwrap_or("*".to_string());
            let peptidoform_str = shared_peptidoform
                .map(|pep| (pep + 1).to_string())
                .unwrap_or("*".to_string());
            let peptide_str = shared_peptide
                .map(|pep| (pep + 1).to_string())
                .unwrap_or("*".to_string());
            let glycan_str = shared_glycan
                .unwrap_or(Some("*".to_string()))
                .unwrap_or_default();
            let loss_str = shared_loss
                .map(|o| o.map(|n| n.hill_notation_html()))
                .unwrap_or(Some("*".to_string()))
                .unwrap_or_default();
            let xl_str = shared_xl.unwrap_or("*".to_string());

            let multi = if annotations.len() > 1 {
                let mut multi = String::new();
                for (annotation, _) in annotations {
                    write!(
                        multi,
                        "{}",
                        get_single_label(
                            annotation,
                            multiple_peptidoforms,
                            multiple_peptides,
                            multiple_glycans
                        )
                    )
                    .unwrap();
                }
                format!("<span class='multi'>{multi}</span>")
            } else {
                String::new()
            };
            let single_internal_glycan =
                matches!(annotations[0].0.ion, FragmentType::Oxonium(_)) && annotations.len() == 1;

            if single_internal_glycan {
                get_single_label(
                    &annotations[0].0,
                    multiple_peptidoforms,
                    multiple_peptides,
                    multiple_glycans,
                )
            } else {
                format!(
                    "<span>{}<sup class='charge'>{}</sup><sub style='--charge-width:{};'><span class='series'>{}</span><span class='glycan-id'>{}</span><span class='peptide-id'>{}</span></sub><span class='neutral-losses'>{}</span><span class='cross-links'>{}</span></span>{}",
                    ion_str,
                    charge_str,
                    charge_str.len(),
                    pos_str,
                    if multiple_glycans {
                        glycan_str
                    } else {
                        String::new()
                    },
                    if multiple_peptidoforms && multiple_peptides {
                        format!("p{}.{}", peptidoform_str, peptide_str)
                    }else if multiple_peptidoforms {
                        format!("p{}", peptidoform_str)
                    } else if multiple_peptides {
                        format!("p{}", peptide_str)
                    } else {
                        String::new()
                    },
                    loss_str,
                    xl_str,
                    multi,

                )
            }
        }
    }
}

fn get_single_label(
    annotation: &Fragment,
    multiple_peptidoforms: bool,
    multiple_peptides: bool,
    multiple_glycans: bool,
) -> String {
    let ch = format!("{:+}", annotation.charge.value);
    format!(
        "<span>{}<sup class='charge'>{}</sup><sub style='--charge-width:{};'><span class='series'>{}</span><span class='glycan-id'>{}</span><span class='peptide-id'>{}</span></sub><span class='neutral-losses'>{}</span><span class='cross-links'>{}</span></span>",
        if let FragmentType::Oxonium(breakages) = &annotation.ion {
            breakages
            .iter()
            .filter(|b| !matches!(b, GlycanBreakPos::End(_)))
            .map(|b| format!(
                "{}<sub>{}</sub>",
                b.label(),
                b.position().label()
            ))
            .join("")
        } else {
            annotation.ion.label().to_string()
        },
        ch,
        ch.len(),
        annotation.ion.position_label().unwrap_or_default(),
        if multiple_glycans {
            if let FragmentType::Oxonium(breakages) = &annotation.ion {
                breakages[0].position().attachment()
            } else {
                annotation.ion.glycan_position().map(|g| g.attachment()).unwrap_or_default()
            }
        } else {
            String::new()
        },
        if multiple_peptidoforms && multiple_peptides {
            format!("p{}.{}", annotation.peptidoform_index+1, annotation.peptide_index + 1)
        }else if multiple_peptidoforms {
            format!("p{}", annotation.peptidoform_index+1)
        } else if multiple_peptides {
            format!("p{}", annotation.peptide_index + 1)
        } else {
            String::new()
        },
        annotation.neutral_loss.as_ref().map(|n| n.hill_notation_html()).unwrap_or_default(),
        get_xl(annotation),
    )
}

fn get_xl(annotation: &Fragment) -> String {
    let bound = annotation
        .formula
        .labels()
        .iter()
        .filter_map(|l| match l {
            AmbiguousLabel::CrossLinkBound(name) => Some(name),
            _ => None,
        })
        .unique()
        .sorted()
        .collect_vec();
    let broken = annotation
        .formula
        .labels()
        .iter()
        .filter_map(|l| match l {
            AmbiguousLabel::CrossLinkBroken(name, _) => Some(name),
            _ => None,
        })
        .unique()
        .sorted()
        .collect_vec();
    let mut output = String::new();
    if !bound.is_empty() {
        write!(
            &mut output,
            "<span class='cs-xl-intact'></span><sub>{}</sub>",
            bound.iter().join(",")
        )
        .unwrap();
    }
    if !broken.is_empty() {
        write!(
            &mut output,
            "<span class='cs-xl-broken'></span><sub>{}</sub>",
            broken.iter().join(",")
        )
        .unwrap();
    }
    output
}