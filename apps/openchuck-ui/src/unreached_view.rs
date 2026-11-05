// SPDX-License-Identifier: AGPL-3.0-or-later
//! Which unreachable areas the view shows, and when (OP-20, UI-11, UI-12;
//! decision record 0515 items 2 and 3; issue #416).
//!
//! The application finds the areas of the latest run on the part's
//! triangles in a job ([`Application::start_unreached_areas`]). On Operations
//! and on Simulate an operation's areas are drawn whenever its toolpath is
//! shown (its eye in the tree is on, and the overall Toolpaths choice), and
//! the result it came from is current: the areas go with the toolpath they
//! were found for.

use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use openchuck_app::project::id::{OperationId, SetupId};
use openchuck_app::unreached::{AreaMarks, SetupAreas};
use openchuck_app::{Application, JobHandle, Regeneration};

use crate::viewport::{ShownArea, ShownOperation, ViewStep, Viewport};

/// The identity of a run, to know it again without holding on to it. The
/// address is only compared while a [`Weak`] to the run is held (in
/// [`Running`] or [`Found`]), so a later run cannot be given the same address
/// and be taken for it.
fn run_key(run: &Arc<Regeneration>) -> usize {
    Arc::as_ptr(run) as usize
}

struct Running {
    run: Weak<Regeneration>,
    setup: SetupId,
    handle: JobHandle<SetupAreas>,
}

struct Found {
    run: Weak<Regeneration>,
    setup: SetupId,
    /// What was found, or nothing: no area, or none that can be placed.
    areas: Option<SetupAreas>,
}

/// What was last handed to the view, to hand it again only when it changes:
/// the operations drawn with the names their legend rows show.
#[derive(Debug, Clone, PartialEq)]
struct Pushed {
    run: usize,
    setup: SetupId,
    operations: Vec<(OperationId, String)>,
}

/// The areas of the run on screen.
#[derive(Default)]
pub struct Areas {
    job: Option<Running>,
    found: Option<Found>,
    /// Which operations' paths are current, for the project revision, run
    /// and setup it was worked out for.
    current: Option<(u64, usize, SetupId, BTreeSet<OperationId>)>,
    pushed: Option<Pushed>,
    /// "Show in view" waiting for the areas to be found.
    request: Option<(OperationId, usize)>,
}

impl Areas {
    /// Works the job and keeps the view's areas right. Returns whether the
    /// frame should be drawn again soon.
    pub fn frame(
        &mut self,
        app: &Application,
        viewport: &mut Viewport,
        run: Option<&Arc<Regeneration>>,
    ) -> bool {
        let setup = viewport.view().setup;
        let step = viewport.step();
        let (Some(run), Some(setup)) = (run, setup) else {
            self.job = None;
            self.found = None;
            self.hand_over(viewport, None, Vec::new());
            return false;
        };
        let key = run_key(run);
        let mut busy = false;
        // Another run or setup: what was found is not about this one.
        if self
            .found
            .as_ref()
            .is_some_and(|f| !Weak::ptr_eq(&f.run, &Arc::downgrade(run)) || f.setup != setup)
        {
            self.found = None;
            self.current = None;
        }
        if self
            .job
            .as_ref()
            .is_some_and(|j| !Weak::ptr_eq(&j.run, &Arc::downgrade(run)) || j.setup != setup)
            && let Some(job) = self.job.take()
        {
            job.handle.cancel();
        }
        if self.found.is_none() && self.job.is_none() {
            let any = run
                .groups
                .iter()
                .any(|g| g.setup == setup && !g.unreached.is_empty());
            if any {
                self.job = Some(Running {
                    run: Arc::downgrade(run),
                    setup,
                    handle: app.start_unreached_areas(Arc::clone(run), setup),
                });
            } else {
                self.found = Some(Found {
                    run: Arc::downgrade(run),
                    setup,
                    areas: None,
                });
            }
        }
        if let Some(job) = &self.job {
            match job.handle.try_result() {
                None => busy = true,
                Some(result) => {
                    let (run_ref, setup) = (job.run.clone(), job.setup);
                    self.job = None;
                    match result {
                        Ok(areas) => {
                            self.found = Some(Found {
                                run: run_ref,
                                setup,
                                areas: Some(areas),
                            });
                        }
                        // The model's file is not the run's, or the job was
                        // stopped: no areas on this part (decision 0492
                        // item 7 for the colours, the same here).
                        Err(_) => {
                            self.found = Some(Found {
                                run: run_ref,
                                setup,
                                areas: None,
                            });
                        }
                    }
                }
            }
        }
        // What is drawn: the operations whose path is shown and current.
        let on_step = matches!(step, ViewStep::Operations | ViewStep::Simulate);
        let toolpaths = viewport.visibility().toolpaths;
        let mut visible: Vec<(OperationId, String)> = Vec::new();
        let mut model = None;
        if on_step
            && toolpaths
            && let Some(found) = &self.found
            && let Some(areas) = &found.areas
        {
            let revision = app.revision();
            if self
                .current
                .as_ref()
                .is_none_or(|(r, k, s, _)| (*r, *k, *s) != (revision, key, setup))
            {
                let current = areas
                    .operations
                    .iter()
                    .filter(|o| !app.operation_unreached(run, o.operation).is_empty())
                    .map(|o| o.operation)
                    .collect();
                self.current = Some((revision, key, setup, current));
            }
            let current = self.current.as_ref().map(|(.., c)| c);
            model = Some(areas.model);
            for operation in &areas.operations {
                if current.is_some_and(|c| c.contains(&operation.operation))
                    && app.project().path_shown(operation.operation)
                {
                    visible.push((
                        operation.operation,
                        operation_name(app, operation.operation),
                    ));
                }
            }
        }
        // Built, with the areas' triangles, only when what is drawn or a
        // name in the legend changed.
        let pushed = Pushed {
            run: key,
            setup,
            operations: visible,
        };
        if self.pushed.as_ref() != Some(&pushed) {
            let shown = self
                .found
                .as_ref()
                .and_then(|f| f.areas.as_ref())
                .map(|areas| {
                    pushed
                        .operations
                        .iter()
                        .filter_map(|(id, name)| {
                            areas
                                .operations
                                .iter()
                                .find(|o| o.operation == *id)
                                .map(|o| shown_operation(*id, name.clone(), &o.areas))
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.pushed = Some(pushed);
            self.hand_over(viewport, model, shown);
        }
        busy || self.job.is_some()
    }

    fn hand_over(
        &mut self,
        viewport: &mut Viewport,
        model: Option<openchuck_app::project::id::ModelId>,
        shown: Vec<ShownOperation>,
    ) {
        if shown.is_empty() {
            self.pushed = None;
        }
        viewport.set_unreached(model, shown);
    }

    /// Asks to look at area `number` (from 1) of `operation` as soon as the
    /// areas are found (Messages' "Show in view N").
    pub fn request(&mut self, operation: OperationId, number: usize) {
        self.request = Some((operation, number));
    }

    /// The area asked for, once it is found and its path is current; the
    /// request is dropped when it cannot be met.
    pub fn take_request(
        &mut self,
        app: &Application,
        run: Option<&Arc<Regeneration>>,
    ) -> Option<(OperationId, AreaMarks)> {
        let (operation, number) = self.request?;
        let run = run?;
        if self.job.is_some() {
            return None;
        }
        self.request = None;
        let found = self
            .found
            .as_ref()
            .filter(|f| Weak::ptr_eq(&f.run, &Arc::downgrade(run)))?;
        if app.operation_unreached(run, operation).is_empty() {
            return None;
        }
        let area = found
            .areas
            .as_ref()?
            .operations
            .iter()
            .find(|o| o.operation == operation)?
            .areas
            .iter()
            .find(|a| a.number == number)?;
        Some((operation, area.clone()))
    }

    /// Whether a request is waiting.
    #[cfg(test)]
    pub fn is_waiting(&self) -> bool {
        self.request.is_some()
    }
}

/// The operation's name as the legend shows it.
fn operation_name(app: &Application, operation: OperationId) -> String {
    app.project()
        .operation(operation)
        .map_or_else(String::new, |o| o.name.clone())
}

fn shown_operation(operation: OperationId, name: String, areas: &[AreaMarks]) -> ShownOperation {
    ShownOperation {
        operation,
        name,
        areas: areas
            .iter()
            .map(|a| ShownArea {
                number: a.number,
                marker: [a.marker.x, a.marker.y, a.marker.z],
                triangles: a.triangles.clone(),
            })
            .collect(),
    }
}
