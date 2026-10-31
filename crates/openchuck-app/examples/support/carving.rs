// SPDX-License-Identifier: AGPL-3.0-or-later
//! The carving project of issue #382 built around an STL, shared by the
//! timing examples `playback_times` and `sim_times` (lanes SIMPERF and
//! SIMSPEED): box stock, rotary roughing with a 3 mm ball, then axial
//! finishing with a 60 degree V-bit with a 0.1 mm tip at a 0.03 mm stepover.

use std::collections::BTreeMap;
use std::path::Path;

use openchuck_app::{Application, Command, Threads};
use openchuck_project::id::{OperationId, SetupId, ToolId};
use openchuck_project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_types::machine::MachineProfile;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::transform::TransformStep;
use openchuck_types::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

/// The peak resident memory of this process so far, in MB (Linux).
pub fn peak_mb() -> f64 {
    let read = || -> Option<f64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
        let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024.0)
    };
    read().unwrap_or(f64::NAN)
}

fn tool(name: &str, shape: ToolShape, diameter: f64, stepover: f64) -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(stepover),
            stepdown: Millimetres(1.0),
        },
    );
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: name.into(),
        tool: Tool {
            shape,
            diameter: Millimetres(diameter),
            flute_length: Millimetres(10.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(diameter.max(3.0)),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

/// A project around `stl` in `dir`: the carving set-up described above.
pub fn carving_project(
    stl: &Path,
    dir: &Path,
    threads: Threads,
) -> Result<Application, Box<dyn std::error::Error>> {
    let _ = std::fs::remove_dir_all(dir);
    let mut app = Application::create(dir, "carving", threads)?;
    std::fs::create_dir_all(dir.join("models"))?;
    std::fs::copy(stl, dir.join("models/part.stl"))?;
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None)?;
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })?;
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))?;
    let setup = SetupId::new();
    let mut s = Setup::new(setup, "Setup 1");
    s.part = Some(model);
    s.material = Some("oak".into());
    s.stock = Some(Stock::Cylinder {
        diameter: Millimetres(28.0),
        length: Millimetres(34.0),
        x_start: Millimetres(-2.0),
    });
    let ball = tool("3 mm ball", ToolShape::BallEnd, 3.0, 1.0);
    let vbit = tool(
        "60 degree V-bit",
        ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.1),
        },
        3.0,
        0.03,
    );
    let operations = [
        ("Roughing", "rotary_roughing", ball.id),
        ("Finishing", "axial_finishing", vbit.id),
    ];
    for item in [Item::Setup(s), Item::Tool(ball), Item::Tool(vbit)] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })?;
    }
    for (name, strategy, tool) in operations {
        app.execute(Command::Add {
            item: Item::Operation(Operation {
                enabled: true,
                id: OperationId::new(),
                order: OrderKey::default(),
                name: name.into(),
                setup,
                strategy: strategy.into(),
                tool: Some(tool),
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                restriction_options: Default::default(),
                spindle_speed: None,
                material: None,
                parameters: BTreeMap::new(),
            }),
            placement: Placement::End,
        })?;
    }
    Ok(app)
}
