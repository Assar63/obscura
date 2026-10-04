//! Feature access through standard V4L2 controls.

use crate::error::Result;
use crate::features::{ChoiceOption, FeatureKind};
use crate::profile::V4l2Binding;
use crate::transport::v4l2::{ControlInfo, VideoNode, CTRL_TYPE_MENU};

/// Refines the catalog kind with the ranges/menus the driver reports.
pub fn describe(info: &ControlInfo, binding: &V4l2Binding, kind: &FeatureKind) -> FeatureKind {
    match kind {
        FeatureKind::Range { scale, unit, .. } => FeatureKind::Range {
            min: info.min - binding.offset,
            max: info.max - binding.offset,
            step: info.step.max(1),
            default: Some(info.default - binding.offset),
            scale: binding.scale.unwrap_or(*scale),
            unit: match &binding.unit {
                Some(u) if u.is_empty() => None,
                Some(u) => Some(u.clone()),
                None => unit.clone(),
            },
        },
        // A menu control bound 1:1 (no value map) takes the driver's labels
        // only if the catalog doesn't define them; catalog labels match the
        // OBSBOT Center wording, so prefer those when the values line up.
        FeatureKind::Choice { options }
            if binding.map.is_empty() && info.type_ == CTRL_TYPE_MENU =>
        {
            let known: Vec<ChoiceOption> = options
                .iter()
                .filter(|o| info.menu.iter().any(|(v, _)| *v == o.value))
                .cloned()
                .collect();
            if known.is_empty() {
                FeatureKind::Choice {
                    options: info
                        .menu
                        .iter()
                        .map(|(value, label)| ChoiceOption {
                            value: *value,
                            label: label.clone(),
                        })
                        .collect(),
                }
            } else {
                FeatureKind::Choice { options: known }
            }
        }
        other => other.clone(),
    }
}

pub fn get(node: &VideoNode, binding: &V4l2Binding) -> Result<i64> {
    let raw = node.get_control(binding.v4l2)?;
    Ok(binding.to_feature(raw))
}

pub fn set(node: &VideoNode, binding: &V4l2Binding, value: i64) -> Result<()> {
    node.set_control(binding.v4l2, binding.to_device(value))
}
