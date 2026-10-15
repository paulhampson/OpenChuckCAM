// SPDX-License-Identifier: AGPL-3.0-or-later
//! Undo and redo (UI-08; work breakdown slice 1 item 6).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};

use openchuck_app::model::{Axis, Degrees, LengthUnit, TransformStep};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    ContentHash, Item, ItemId, ListKind, ModelRef, Operation, OrderKey, ParameterValue, Placement,
    Project, RelativePath, SelectionSet, SelectionTarget, Setup, ToolSnapshot, file,
};
use openchuck_app::setup::{Clearance, MachineProfile, Stock, WorkOffsetRegister};
use openchuck_app::{AppError, Application, Command, Threads};
use openchuck_types::brep::FaceId;
use openchuck_types::tool::{ControllerMapping, Tool, ToolShape};
use openchuck_types::units::Millimetres;
use proptest::prelude::*;

fn open_app(name: &str) -> Application {
    let dir = std::env::temp_dir().join(format!("openchuck-undo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Application::create(&dir, name, Threads::Exactly(1)).unwrap()
}

fn tool(diameter: f64) -> Tool {
    Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(25.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
    }
}

/// One random edit, chosen against the project as it is when the edit runs.
#[derive(Debug, Clone)]
enum Edit {
    Renumber(usize),
    Rename(String),
    Machine(Option<String>),
    AddSetup(Option<usize>),
    AddTool(f64),
    AddModel,
    AddSelection {
        pick: usize,
        faces: Vec<u32>,
    },
    AddOperation {
        pick: usize,
        index: Option<usize>,
    },
    Remove {
        kind: usize,
        pick: usize,
    },
    Move {
        kind: usize,
        pick: usize,
        index: usize,
    },
    SetParameter {
        pick: usize,
        name: usize,
        value: Option<f64>,
    },
    SetTool {
        pick: usize,
        tool: Option<usize>,
    },
    SetEnabled {
        pick: usize,
        enabled: bool,
    },
    Duplicate(usize),
    RenameSetup {
        pick: usize,
        name: String,
    },
    ChangeSetup {
        pick: usize,
        part: Option<usize>,
        stock: Option<f64>,
        offset: f64,
        clearance: Option<f64>,
    },
    SetModelUnits {
        pick: usize,
        unit: Option<usize>,
    },
    SetModelTransform {
        pick: usize,
        steps: Vec<(usize, f64)>,
    },
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        "[a-z]{0,8}".prop_map(Edit::Rename),
        proptest::option::of("[a-z]{1,8}").prop_map(Edit::Machine),
        proptest::option::of(0usize..4).prop_map(Edit::AddSetup),
        (1.0f64..20.0).prop_map(Edit::AddTool),
        Just(Edit::AddModel),
        (any::<usize>(), proptest::collection::vec(0u32..50, 0..4))
            .prop_map(|(pick, faces)| Edit::AddSelection { pick, faces }),
        (any::<usize>(), proptest::option::of(0usize..4))
            .prop_map(|(pick, index)| Edit::AddOperation { pick, index }),
        (0usize..5, any::<usize>()).prop_map(|(kind, pick)| Edit::Remove { kind, pick }),
        (0usize..5, any::<usize>(), 0usize..4).prop_map(|(kind, pick, index)| Edit::Move {
            kind,
            pick,
            index
        }),
        (
            any::<usize>(),
            0usize..3,
            proptest::option::of(-100.0f64..100.0)
        )
            .prop_map(|(pick, name, value)| Edit::SetParameter { pick, name, value }),
        (any::<usize>(), proptest::option::of(any::<usize>()))
            .prop_map(|(pick, tool)| Edit::SetTool { pick, tool }),
        (any::<usize>(), any::<bool>())
            .prop_map(|(pick, enabled)| Edit::SetEnabled { pick, enabled }),
        any::<usize>().prop_map(Edit::Duplicate),
        (any::<usize>(), "[A-Z][a-z]{0,6}")
            .prop_map(|(pick, name)| Edit::RenameSetup { pick, name }),
        (
            any::<usize>(),
            proptest::option::of(any::<usize>()),
            proptest::option::of(1.0f64..80.0),
            -50.0f64..50.0,
            proptest::option::of(1.0f64..80.0),
        )
            .prop_map(|(pick, part, stock, offset, clearance)| Edit::ChangeSetup {
                pick,
                part,
                stock,
                offset,
                clearance
            }),
        (0usize..5).prop_map(Edit::Renumber),
        (any::<usize>(), proptest::option::of(0usize..4))
            .prop_map(|(pick, unit)| Edit::SetModelUnits { pick, unit }),
        (
            any::<usize>(),
            proptest::collection::vec((0usize..5, -100.0f64..100.0), 0..4)
        )
            .prop_map(|(pick, steps)| Edit::SetModelTransform { pick, steps }),
    ]
}

fn placement(index: Option<usize>) -> Placement {
    index.map_or(Placement::End, Placement::Index)
}

fn pick<T>(items: &[T], pick: usize) -> Option<&T> {
    if items.is_empty() {
        None
    } else {
        items.get(pick % items.len())
    }
}

fn item_ids(p: &Project, kind: usize) -> Vec<ItemId> {
    match kind {
        0 => p.models().iter().map(|m| ItemId::Model(m.id)).collect(),
        1 => p.setups().iter().map(|s| ItemId::Setup(s.id)).collect(),
        2 => p.tools().iter().map(|t| ItemId::Tool(t.id)).collect(),
        3 => p
            .selection_sets()
            .iter()
            .map(|s| ItemId::SelectionSet(s.id))
            .collect(),
        _ => p
            .operations()
            .iter()
            .map(|o| ItemId::Operation(o.id))
            .collect(),
    }
}

/// Turns an edit into a command for the current project, or `None` when it
/// has nothing to act on. `next` supplies fresh, repeatable IDs.
fn command(p: &Project, edit: Edit, next: &mut u128) -> Option<Command> {
    *next += 1;
    let id = *next;
    Some(match edit {
        Edit::Rename(name) => Command::RenameProject(name),
        Edit::Machine(name) => Command::SetMachine(name.map(|name| {
            let mut profile = MachineProfile::example_x_rotary();
            profile.name = name;
            Box::new(profile)
        })),
        Edit::AddSetup(index) => Command::Add {
            item: Item::Setup(Setup::new(SetupId::from_u128(id), format!("Setup {id}"))),
            placement: placement(index),
        },
        Edit::AddTool(diameter) => Command::Add {
            item: Item::Tool(ToolSnapshot {
                id: ToolId::from_u128(id),
                order: OrderKey::default(),
                name: format!("{diameter} mm"),
                tool: tool(diameter),
                controller: ControllerMapping::numbered(1),
                cutting_data: Default::default(),
            }),
            placement: Placement::End,
        },
        Edit::AddModel => Command::Add {
            // STEP, because the generated selections select faces.
            item: Item::Model(ModelRef::new(
                ModelId::from_u128(id),
                RelativePath::new(format!("models/m{id}.step")).unwrap(),
                ContentHash::of_bytes(&id.to_le_bytes()),
            )),
            placement: Placement::End,
        },
        Edit::AddSelection { pick: n, faces } => {
            let model = pick(p.models(), n)?;
            Command::Add {
                item: Item::SelectionSet(SelectionSet {
                    id: SelectionSetId::from_u128(id),
                    order: OrderKey::default(),
                    name: format!("Set {id}"),
                    model: model.id,
                    model_hash: model.hash,
                    target: SelectionTarget::Faces(
                        faces.into_iter().map(FaceId).collect::<BTreeSet<_>>(),
                    ),
                    descriptors: Vec::new(),
                }),
                placement: Placement::End,
            }
        }
        Edit::AddOperation { pick: n, index } => {
            let setup = pick(p.setups(), n)?;
            Command::Add {
                item: Item::Operation(Operation {
                    enabled: true,
                    id: OperationId::from_u128(id),
                    order: OrderKey::default(),
                    name: format!("Op {id}"),
                    setup: setup.id,
                    strategy: "helical_finish".into(),
                    tool: pick(p.tools(), n).map(|t| t.id),
                    selections: pick(p.selection_sets(), n)
                        .map(|s| s.id)
                        .into_iter()
                        .collect(),
                    parameters: BTreeMap::new(),
                }),
                placement: placement(index),
            }
        }
        Edit::Remove { kind, pick: n } => Command::Remove(*pick(&item_ids(p, kind), n)?),
        Edit::Move {
            kind,
            pick: n,
            index,
        } => Command::Move {
            id: *pick(&item_ids(p, kind), n)?,
            index,
        },
        Edit::SetParameter {
            pick: n,
            name,
            value,
        } => Command::SetParameter {
            operation: pick(p.operations(), n)?.id,
            name: ["stepover", "depth", "angle"][name].into(),
            value: value.map(ParameterValue::Number),
        },
        Edit::SetEnabled { pick: n, enabled } => Command::SetOperationEnabled {
            operation: pick(p.operations(), n)?.id,
            enabled,
        },
        Edit::Duplicate(n) => {
            openchuck_app::duplicate_operation(p, pick(p.operations(), n)?.id).ok()?
        }
        Edit::SetTool { pick: n, tool } => {
            let mut op = pick(p.operations(), n)?.clone();
            op.tool = tool.and_then(|t| pick(p.tools(), t)).map(|t| t.id);
            Command::Replace(Item::Operation(op))
        }
        Edit::Renumber(kind) => Command::Renumber(
            [
                ListKind::Models,
                ListKind::Setups,
                ListKind::Tools,
                ListKind::SelectionSets,
                ListKind::Operations,
            ][kind],
        ),
        Edit::RenameSetup { pick: n, name } => {
            let mut setup = pick(p.setups(), n)?.clone();
            setup.name = name;
            Command::Replace(Item::Setup(setup))
        }
        Edit::ChangeSetup {
            pick: n,
            part,
            stock,
            offset,
            clearance,
        } => {
            let mut setup = pick(p.setups(), n)?.clone();
            setup.part = match part {
                Some(m) => Some(pick(p.models(), m)?.id),
                None => None,
            };
            setup.stock = stock.map(|d| Stock::Cylinder {
                diameter: Millimetres(d),
                length: Millimetres(2.0 * d),
                x_start: Millimetres(offset),
            });
            setup.material = stock.map(|d| format!("material {d}"));
            setup.frame.part_offset = Millimetres(offset);
            setup.frame.work_offset.x = Millimetres(-offset);
            setup.frame.work_offset.register = WorkOffsetRegister::ALL[n % 6];
            setup.clearance = clearance.map(|r| Clearance {
                radius: Millimetres(r),
                safe_z: Millimetres(r + 10.0),
            });
            Command::Replace(Item::Setup(setup))
        }
        Edit::SetModelUnits { pick: n, unit } => Command::SetModelUnits {
            model: pick(p.models(), n)?.id,
            units: unit.map(|u| LengthUnit::ALL[u]),
        },
        Edit::SetModelTransform { pick: n, steps } => Command::SetModelTransform {
            model: pick(p.models(), n)?.id,
            transform: steps
                .into_iter()
                .map(|(kind, v)| match kind {
                    0 => TransformStep::Translate {
                        x: v,
                        y: -v,
                        z: 1.0,
                    },
                    1 => TransformStep::Rotate {
                        axis: Axis::Z,
                        angle: Degrees(v),
                    },
                    // Some factors are invalid, so refused commands are exercised too.
                    2 => TransformStep::Scale { factor: v },
                    3 => TransformStep::Mirror { axis: Axis::Y },
                    _ => TransformStep::CentreOnRotaryAxis,
                })
                .collect(),
        },
    })
}

/// Applies the edits that are valid on the project as it goes, and returns how
/// many were recorded.
fn run(app: &mut Application, edits: Vec<Edit>, next: &mut u128) -> usize {
    let mut recorded = 0;
    for edit in edits {
        let Some(command) = command(app.project(), edit, next) else {
            continue;
        };
        let before = app.project().clone();
        match app.execute(command) {
            Ok(()) => recorded += 1,
            // A refused edit (for example removing a setup in use) changes nothing.
            Err(AppError::Project(_)) => assert_eq!(app.project(), &before),
            Err(e) => panic!("unexpected error {e}"),
        }
        app.project().validate().unwrap();
    }
    recorded
}

/// The project as saved: every file's path and bytes (verification plan,
/// UI-08: "byte for byte").
fn snapshot(p: &Project) -> String {
    format!("{:?}", file::render(p).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]


    #[test]
    fn req_ui_08_full_undo_restores_and_redo_reapplies(
        setup in proptest::collection::vec(edit(), 0..20),
        edits in proptest::collection::vec(edit(), 0..40),
    ) {
        let mut app = open_app("prop");
        let mut next = 0;
        run(&mut app, setup, &mut next);
        let original = snapshot(app.project());

        let recorded = run(&mut app, edits, &mut next);
        let edited = snapshot(app.project());

        for _ in 0..recorded {
            prop_assert!(app.undo().unwrap());
        }
        prop_assert_eq!(snapshot(app.project()), original);

        for _ in 0..recorded {
            prop_assert!(app.redo().unwrap());
        }
        prop_assert!(!app.can_redo());
        prop_assert_eq!(snapshot(app.project()), edited);
    }
}

#[test]
fn req_ui_08_new_command_clears_redo_and_failed_command_is_not_recorded() {
    let mut app = open_app("redo");
    assert!(!app.undo().unwrap());
    assert!(!app.redo().unwrap());

    let setup = SetupId::from_u128(1);
    app.execute(Command::Add {
        item: Item::Setup(Setup::new(setup, "Setup 1")),
        placement: Placement::End,
    })
    .unwrap();
    app.execute(Command::RenameProject("renamed".into()))
        .unwrap();
    assert!(app.undo().unwrap());
    assert!(app.can_redo());
    app.execute(Command::RenameProject("other".into())).unwrap();
    assert!(!app.can_redo());

    // Removing an unknown item fails, changes nothing and is not recorded.
    let before = app.project().clone();
    let err = app
        .execute(Command::Remove(ItemId::Setup(SetupId::from_u128(99))))
        .unwrap_err();
    assert!(err.to_string().contains("no setup"), "{err}");
    assert_eq!(app.project(), &before);
    assert!(app.undo().unwrap()); // undoes the rename to "other"
    assert_eq!(app.project().setups().len(), 1);
    assert!(app.undo().unwrap()); // undoes adding the setup
    assert!(app.project().setups().is_empty());
    assert!(!app.can_undo());
}
