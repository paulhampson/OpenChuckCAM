// SPDX-License-Identifier: AGPL-3.0-or-later
//! A deferred scene selection names geometry, never an old scene index or
//! hit position. Keep may replace the model or rebuild its placement.

use eframe::egui::Modifiers;
use openchuck_app::{
    Application,
    pick::{EdgeRef, FaceId, SceneEdgePick, ScenePick},
    project::{
        ContentHash,
        id::{ModelId, SetupId},
    },
    scene::{SceneView, WorkholdingItem},
    setup::Workholding,
};

use crate::viewport::PickInput;

#[derive(Clone)]
pub(super) enum Intent {
    Surface {
        model: ModelId,
        hash: ContentHash,
        triangle: u32,
        face: Option<FaceId>,
        modifiers: Modifiers,
    },
    Edge {
        model: ModelId,
        hash: ContentHash,
        edge: EdgeRef,
        vertices: Option<[u32; 2]>,
        modifiers: Modifiers,
    },
    Solid {
        setup: SetupId,
        item: WorkholdingItem,
    },
}

fn hash(app: &Application, model: ModelId) -> Option<ContentHash> {
    app.project()
        .models()
        .iter()
        .find(|m| m.id == model)
        .map(|m| m.hash)
}

impl Intent {
    pub(super) fn capture(app: &Application, view: &SceneView, input: PickInput) -> Option<Self> {
        if let Some(item) = input.solid_click {
            return Some(Self::Solid {
                setup: view.setup?,
                item,
            });
        }
        if let Some((pick, modifiers)) = input.edge_click {
            let model = pick.model?;
            return Some(Self::Edge {
                model,
                hash: hash(app, model)?,
                edge: pick.edge,
                vertices: view
                    .meshes
                    .get(pick.mesh)?
                    .feature_edges
                    .get(pick.index)
                    .copied(),
                modifiers,
            });
        }
        let (pick, modifiers) = input.click?;
        let model = pick.model?;
        Some(Self::Surface {
            model,
            hash: hash(app, model)?,
            triangle: pick.triangle,
            face: pick.face,
            modifiers,
        })
    }

    pub(super) fn resolve(
        &self,
        app: &Application,
        view: &SceneView,
        draft: Option<(SetupId, &Workholding)>,
    ) -> Option<PickInput> {
        let mut input = PickInput::default();
        let (model, original_hash) = match *self {
            Self::Surface { model, hash, .. } | Self::Edge { model, hash, .. } => (model, hash),
            Self::Solid { setup, item } => {
                if view.setup != Some(setup) {
                    return None;
                }
                let workholding = match draft.filter(|(id, _)| *id == setup) {
                    Some((_, workholding)) => workholding,
                    None => {
                        &app.project()
                            .setups()
                            .iter()
                            .find(|s| s.id == setup)?
                            .workholding
                    }
                };
                let exists = match item {
                    WorkholdingItem::Start => workholding.start.is_some(),
                    WorkholdingItem::End => workholding.end.is_some(),
                    WorkholdingItem::Tab(i) => workholding.tabs.get(i).is_some(),
                };
                if exists {
                    input.solid_click = Some(item);
                    return Some(input);
                }
                return None;
            }
        };
        if hash(app, model) != Some(original_hash) {
            return None;
        }
        let (mesh, item) = view
            .meshes
            .iter()
            .enumerate()
            .find(|(_, m)| m.model == Some(model))?;
        match *self {
            Self::Surface {
                triangle,
                face,
                modifiers,
                ..
            } => {
                let triangle = if let Some(face) = face {
                    let faces = &item.brep.as_ref()?.faces;
                    if faces.get(triangle as usize) == Some(&face) {
                        triangle
                    } else {
                        u32::try_from(faces.iter().position(|f| *f == face)?).ok()?
                    }
                } else {
                    triangle
                };
                let [a, b, c] = item.mesh.triangle(triangle as usize)?;
                let normal = (b - a).cross(c - a);
                let length = normal.length();
                input.click = Some((
                    ScenePick {
                        mesh,
                        model: Some(model),
                        triangle,
                        face,
                        distance: 0.0,
                        point: a,
                        normal: (length > 0.0).then(|| normal * (1.0 / length)),
                    },
                    modifiers,
                ));
            }
            Self::Edge {
                edge,
                vertices,
                modifiers,
                ..
            } => {
                let (index, point) = match edge {
                    EdgeRef::Brep(id) => {
                        let (i, edge) = item
                            .brep
                            .as_ref()?
                            .edges
                            .iter()
                            .enumerate()
                            .find(|(_, e)| e.id == id)?;
                        (i, *edge.points.first()?)
                    }
                    EdgeRef::Mesh(_) => {
                        let vertices = vertices?;
                        let i = item
                            .feature_edges
                            .iter()
                            .position(|v| *v == vertices || *v == [vertices[1], vertices[0]])?;
                        (i, *item.mesh.vertices.get(vertices[0] as usize)?)
                    }
                };
                input.edge_click = Some((
                    SceneEdgePick {
                        mesh,
                        model: Some(model),
                        edge,
                        index,
                        distance: 0.0,
                        point,
                    },
                    modifiers,
                ));
            }
            Self::Solid { .. } => return None,
        }
        Some(input)
    }
}
