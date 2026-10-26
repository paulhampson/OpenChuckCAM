// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine section of the setup inspector as a view model: each machine
//! profile value in words, whether it is confirmed on the machine, where it
//! came from and how to confirm it (UI-13, SET-01; interface specification
//! sections 8 and 11.3; decision record 0029 item 16).
//!
//! The profile records two states, confirmed and not (`Confirmation`), and the
//! owner chose two icons on them (decision record 0114 section F answer 4,
//! OI-339; the UI-13 wording that follows is proposed as OI-345). Where the
//! value came from is the profile's own source text, shown in the tooltip.

use crate::setup::{ARange, MachineProfile, ProfileField, a_direction};

/// One machine value as the inspector shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineValue {
    /// Which value of the profile it is.
    pub field: ProfileField,
    /// The label, in sentence case.
    pub label: &'static str,
    /// The value in words, with its unit.
    pub value: String,
    /// True when it is confirmed on the machine (the green tick); false for
    /// the amber triangle.
    pub confirmed: bool,
    /// The tooltip: confirmed or not, where the value came from, and, when it
    /// is not confirmed, how to confirm it.
    pub tooltip: String,
    /// Shown in the collapsed "Travel and maximum feeds" part (11.3).
    pub collapsed: bool,
}

/// How to confirm `field` on the machine, for the tooltip of a value that is
/// not confirmed.
pub fn how_to_confirm(field: ProfileField) -> &'static str {
    match field {
        ProfileField::RotaryAxis => {
            "Check which machine axis the chuck turns about, and mark it confirmed in the profile."
        }
        ProfileField::ASense => {
            "Send a small A+ move with a mark on the chuck and see which way its top turns."
        }
        ProfileField::RotaryOrigin => {
            "Measure it by touching the tool tip to the chuck centre and reading G53 Z."
        }
        ProfileField::ARange => {
            "Turn A past a full turn each way and see whether the controller stops it."
        }
        ProfileField::Travel => {
            "Jog each axis to its limits and read the machine coordinates (G53)."
        }
        ProfileField::MaxFeeds => {
            "Take them from the controller's settings or the machine's documented limits."
        }
        ProfileField::Clearance => "Set a clearance on the setup to suit its stock.",
        ProfileField::MinBlockMs => {
            "Run the block-rate air cut and record the shortest smooth setting, firmware and program delivery method."
        }
        ProfileField::BlockWindow => {
            "Take the planner queue depth from controller documentation; a value of 1 checks each block."
        }
        ProfileField::ShortBlockMode => {
            "Choose Report to inspect demand or Slow down to preserve geometry at a lower feed."
        }
        ProfileField::SpindleRange => {
            "Check the spindle's lowest and highest speed in the machine's documents or controller settings."
        }
    }
}

/// The tooltip of `field` in `profile` (section 8): confirmed or not, the
/// source the profile records, and how to confirm a value that is not.
pub fn tooltip(profile: &MachineProfile, field: ProfileField) -> String {
    let source = profile
        .provenance
        .get(&field)
        .map(|p| p.source.trim().trim_end_matches('.'))
        .filter(|s| !s.is_empty());
    if field == ProfileField::ShortBlockMode {
        return match source {
            Some(source) => format!(
                "Posting policy. Source: {source}. {}",
                how_to_confirm(field)
            ),
            None => format!("Posting policy. {}", how_to_confirm(field)),
        };
    }
    let source = match source {
        Some(s) => format!("Source: {s}."),
        None => "No source is recorded, so it is treated as an unconfirmed placeholder.".to_owned(),
    };
    if profile.is_confirmed(field) {
        format!("Confirmed on the machine. {source}")
    } else {
        format!(
            "Not confirmed on the machine. {source} {}",
            how_to_confirm(field)
        )
    }
}

fn degrees(v: f64) -> String {
    format!("{v:.2}\u{b0}")
}

/// The values of the machine section, in the order of 11.3: rotary axis, A
/// range, A+ direction, axis height, then travel, the maximum feeds and,
/// when the profile gives one, the spindle speed range (collapsed).
pub fn machine_values(profile: &MachineProfile) -> Vec<MachineValue> {
    let value = |field, label, value: String, collapsed| MachineValue {
        field,
        label,
        value,
        confirmed: profile.is_confirmed(field),
        tooltip: tooltip(profile, field),
        collapsed,
    };
    let a_range = match profile.a_range {
        ARange::Continuous => "Continuous".to_owned(),
        ARange::Limited { min, max } => format!("{} to {}", degrees(min.0), degrees(max.0)),
    };
    let direction = a_direction(profile);
    let mut direction = direction.replacen("positive A", "A+", 1);
    if let Some(first) = direction.get(..1) {
        direction = format!(
            "{}{}",
            first.to_uppercase(),
            direction.get(1..).unwrap_or("")
        );
    }
    let t = &profile.travel;
    let f = &profile.max_feeds;
    let spindle = match (profile.min_spindle_rpm, profile.max_spindle_rpm) {
        (Some(min), Some(max)) => Some(format!("{:.0} to {:.0} rpm", min.0, max.0)),
        (None, Some(max)) => Some(format!("Up to {:.0} rpm", max.0)),
        (Some(min), None) => Some(format!("From {:.0} rpm", min.0)),
        (None, None) => None,
    };
    let mut values = vec![
        value(
            ProfileField::RotaryAxis,
            "Rotary axis",
            format!("A, about {}", profile.rotary_axis.name().to_uppercase()),
            false,
        ),
        value(ProfileField::ARange, "A range", a_range, false),
        value(ProfileField::ASense, "A+ direction", direction, false),
        value(
            ProfileField::RotaryOrigin,
            "Axis height",
            format!("{:.3} mm (machine Z)", profile.rotary_origin.z),
            false,
        ),
        value(
            ProfileField::Travel,
            "Travel",
            format!(
                "X {:.3} to {:.3} mm, Y {:.3} to {:.3} mm, Z {:.3} to {:.3} mm",
                t.min.x, t.max.x, t.min.y, t.max.y, t.min.z, t.max.z
            ),
            true,
        ),
        value(
            ProfileField::MaxFeeds,
            "Maximum feeds",
            format!(
                "X and Y {:.0} mm/min, Z {:.0} mm/min, A {:.0} \u{b0}/min",
                f.linear.0, f.z.0, f.rotary.0
            ),
            true,
        ),
    ];
    // Only a profile that gives a range shows one (decision record 0231).
    if let Some(range) = spindle {
        values.push(value(
            ProfileField::SpindleRange,
            "Spindle speed",
            range,
            true,
        ));
    }
    let hardware_fields = profile.fields();
    values.extend(
        [
            value(
                ProfileField::MinBlockMs,
                "Minimum block time",
                format!("{} ms (0 is off)", profile.min_block_ms),
                false,
            ),
            value(
                ProfileField::BlockWindow,
                "Block window",
                format!(
                    "{} block{}",
                    profile.block_window,
                    if profile.block_window == 1 { "" } else { "s" }
                ),
                false,
            ),
            value(
                ProfileField::ShortBlockMode,
                "Short-block mode",
                profile.short_block_mode.label().into(),
                false,
            ),
        ]
        .into_iter()
        .filter(|value| {
            value.field == ProfileField::ShortBlockMode || hardware_fields.contains(&value.field)
        }),
    );
    values
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::setup::{Confirmation, MAKERA_Z1, Provenance, builtin};

    #[test]
    fn req_ui_13_each_value_says_whether_it_is_confirmed_its_source_and_how_to_confirm() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        let values = machine_values(&z1);
        let labels: Vec<&str> = values.iter().map(|v| v.label).collect();
        assert_eq!(
            labels,
            [
                "Rotary axis",
                "A range",
                "A+ direction",
                "Axis height",
                "Travel",
                "Maximum feeds",
                "Spindle speed",
                "Minimum block time",
                "Block window",
                "Short-block mode"
            ]
        );
        let spindle = values.iter().find(|v| v.label == "Spindle speed").unwrap();
        assert_eq!(spindle.value, "Up to 13000 rpm");
        assert!(
            !spindle.confirmed,
            "from Makera's documents, not the machine"
        );
        // The Z1's axis height is a placeholder: the amber triangle, with
        // where it came from and how to confirm it.
        let height = values.iter().find(|v| v.label == "Axis height").unwrap();
        assert!(!height.confirmed);
        assert!(height.value.ends_with("mm (machine Z)"), "{}", height.value);
        assert!(height.tooltip.starts_with("Not confirmed on the machine."));
        assert!(height.tooltip.contains("Source: "));
        assert!(height.tooltip.contains("G53 Z"));
        // A+ is confirmed: the tick, with no instructions.
        let sense = values.iter().find(|v| v.label == "A+ direction").unwrap();
        assert!(sense.confirmed);
        assert!(sense.tooltip.starts_with("Confirmed on the machine."));
        assert!(!sense.tooltip.contains("G53"));
        assert!(sense.value.starts_with("A+ turns"), "{}", sense.value);
        // No file field names in the words (section 15).
        for v in &values {
            assert!(!v.tooltip.contains("rotary_origin"), "{}", v.tooltip);
            assert!(!v.value.contains('_'), "{}", v.value);
        }
        let mode = values
            .iter()
            .find(|v| v.field == ProfileField::ShortBlockMode)
            .unwrap();
        assert!(mode.tooltip.starts_with("Posting policy."));
        assert!(!mode.tooltip.contains("confirmed on the machine"));
        assert!(values.iter().filter(|v| v.collapsed).count() == 3);
    }

    #[test]
    fn req_ui_13_a_value_with_no_source_is_an_unconfirmed_placeholder() {
        let mut z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        z1.provenance.remove(&ProfileField::Travel);
        assert!(tooltip(&z1, ProfileField::Travel).contains("unconfirmed placeholder"));
        z1.provenance.insert(
            ProfileField::Travel,
            Provenance {
                state: Confirmation::Confirmed,
                source: "measured 1 Oct 2026".to_owned(),
            },
        );
        assert_eq!(
            tooltip(&z1, ProfileField::Travel),
            "Confirmed on the machine. Source: measured 1 Oct 2026."
        );
    }
}
