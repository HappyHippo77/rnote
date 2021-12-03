use super::{StrokeKey, StrokeStyle, StrokesState};
use crate::render;
use crate::strokes::strokestyle::StrokeBehaviour;
use crate::ui::canvas;

use gtk4::{gdk, graphene, gsk, Snapshot};
use p2d::bounding_volume::BoundingVolume;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderComponent {
    pub render: bool,
    #[serde(skip, default = "render::default_rendernode")]
    pub rendernode: gsk::RenderNode,
}

impl Default for RenderComponent {
    fn default() -> Self {
        Self {
            render: true,
            rendernode: render::default_rendernode(),
        }
    }
}

impl StrokesState {
    /// Returns false if rendering is not supported
    pub fn can_render(&self, key: StrokeKey) -> bool {
        self.render_components.get(key).is_some()
    }

    /// Wether rendering is enabled. Returns None if rendering is not supported
    pub fn does_render(&self, key: StrokeKey) -> Option<bool> {
        if let Some(render_comp) = self.render_components.get(key) {
            Some(render_comp.render)
        } else {
            log::warn!(
                "failed to get render_comp of stroke for key {:?}, invalid key used or stroke does not support rendering",
                key
            );
            None
        }
    }

    pub fn set_render(&mut self, key: StrokeKey, render: bool) {
        if let Some(render_component) = self.render_components.get_mut(key) {
            render_component.render = render;
        } else {
            log::warn!(
                "failed to get render_comp of stroke with key {:?}, invalid key used or stroke does not support rendering",
                key
            );
        }
    }

    pub fn update_rendering_newest_stroke(&mut self) {
        let last_stroke_key = self.last_stroke_key();
        if let Some(key) = last_stroke_key {
            self.update_rendering_for_stroke(key);
        }
    }

    pub fn update_rendering_newest_selected(&mut self) {
        let last_selection_key = self.last_selection_key();

        if let Some(last_selection_key) = last_selection_key {
            self.update_rendering_for_stroke(last_selection_key);
        }
    }

    pub fn update_rendering(&mut self, viewport: Option<p2d::bounding_volume::AABB>) {
        let keys = self.render_components.keys().collect::<Vec<StrokeKey>>();

        keys.iter().for_each(|key| {
            if let (Some(stroke), Some(render_comp)) =
                (self.strokes.get(*key), self.render_components.get_mut(*key))
            {
                // skip if stroke is not in viewport
                if let Some(viewport) = viewport {
                    if !viewport.intersects(&stroke.bounds()) {
                        return;
                    }
                }

                match stroke.gen_rendernode(self.scalefactor, &self.renderer) {
                    Ok(Some(node)) => {
                        render_comp.rendernode = node;
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to generate rendernode for stroke with key: {:?}, {}",
                            key,
                            e
                        )
                    }
                    _ => {}
                }
            } else {
                log::warn!(
                    "failed to get stroke with key {:?}, invalid key used or stroke does not support rendering",
                    key
                );
            }
        })
    }

    pub fn update_rendering_for_stroke(&mut self, key: StrokeKey) {
        if let (Some(stroke), Some(render_comp)) =
            (self.strokes.get(key), self.render_components.get_mut(key))
        {
            match stroke.gen_rendernode(self.scalefactor, &self.renderer) {
                Ok(Some(node)) => {
                    render_comp.rendernode = node;
                }
                Err(e) => {
                    log::error!(
                        "Failed to generate rendernode for stroke with key: {:?}, {}",
                        key,
                        e
                    )
                }
                _ => {}
            }
        } else {
            log::warn!(
                "failed to get stroke with key {:?}, invalid key used or stroke does not support rendering",
                key
            );
        }
    }

    pub fn update_rendering_for_selection(&mut self) {
        let selection_keys = self.selection_keys();

        selection_keys.iter().for_each(|key| {
            self.update_rendering_for_stroke(*key);
        });
    }

    pub fn draw_strokes(&self, snapshot: &Snapshot, viewport: Option<p2d::bounding_volume::AABB>) {
        self.render_components
            .iter()
            .filter(|(key, render_comp)| {
                render_comp.render && !(self.trashed(*key).unwrap_or_else(|| true))
            })
            .for_each(|(key, render_comp)| {
                if let Some(stroke) = self.strokes.get(key) {
                    // skip if stroke is not in viewport
                    if let Some(viewport) = viewport {
                        if !viewport.intersects(&stroke.bounds()) {
                            return;
                        }
                    }
                    snapshot.append_node(&render_comp.rendernode);
                }
            });
    }

    pub fn draw_selection(&self, scalefactor: f64, snapshot: &Snapshot) {
        fn draw_selected_bounds(
            bounds: p2d::bounding_volume::AABB,
            scalefactor: f64,
            snapshot: &Snapshot,
        ) {
            let bounds = graphene::Rect::new(
                bounds.mins[0] as f32,
                bounds.mins[1] as f32,
                (bounds.maxs[0] - bounds.mins[0]) as f32,
                (bounds.maxs[1] - bounds.mins[1]) as f32,
            )
            .scale(scalefactor as f32, scalefactor as f32);
            let border_color = gdk::RGBA {
                red: 0.0,
                green: 0.2,
                blue: 0.8,
                alpha: 0.4,
            };
            let border_width = 2.0;

            snapshot.append_border(
                &gsk::RoundedRect::new(
                    graphene::Rect::new(bounds.x(), bounds.y(), bounds.width(), bounds.height()),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                ),
                &[border_width, border_width, border_width, border_width],
                &[border_color, border_color, border_color, border_color],
            );
        }

        self.render_components
            .iter()
            .filter(|(key, render_comp)| {
                render_comp.render
                    && !(self.trashed(*key).unwrap_or_else(|| false))
                    && (self.selected(*key).unwrap_or_else(|| false))
            })
            .for_each(|(key, render_comp)| {
                snapshot.append_node(&render_comp.rendernode);
                if let Some(selection_comp) = self.selection_components.get(key) {
                    if selection_comp.selected {
                        if let Some(stroke) = self.strokes.get(key) {
                            draw_selected_bounds(stroke.bounds(), scalefactor, snapshot);
                        }
                    }
                }
            });
        self.draw_selection_bounds(scalefactor, snapshot);
    }

    pub fn draw_selection_bounds(&self, scalefactor: f64, snapshot: &Snapshot) {
        if let Some(selection_bounds) = self.selection_bounds {
            let selection_bounds = graphene::Rect::new(
                selection_bounds.mins[0] as f32,
                selection_bounds.mins[1] as f32,
                (selection_bounds.maxs[0] - selection_bounds.mins[0]) as f32,
                (selection_bounds.maxs[1] - selection_bounds.mins[1]) as f32,
            )
            .scale(scalefactor as f32, scalefactor as f32);

            let selection_border_color = gdk::RGBA {
                red: 0.49,
                green: 0.56,
                blue: 0.63,
                alpha: 0.3,
            };
            let selection_border_width = 4.0;

            snapshot.append_color(
                &gdk::RGBA {
                    red: 0.49,
                    green: 0.56,
                    blue: 0.63,
                    alpha: 0.1,
                },
                &selection_bounds,
            );
            snapshot.append_border(
                &gsk::RoundedRect::new(
                    graphene::Rect::new(
                        selection_bounds.x(),
                        selection_bounds.y(),
                        selection_bounds.width(),
                        selection_bounds.height(),
                    ),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                    graphene::Size::zero(),
                ),
                &[
                    selection_border_width,
                    selection_border_width,
                    selection_border_width,
                    selection_border_width,
                ],
                &[
                    selection_border_color,
                    selection_border_color,
                    selection_border_color,
                    selection_border_color,
                ],
            );
        }
    }

    pub fn draw_debug(&self, scalefactor: f64, snapshot: &Snapshot) {
        self.strokes.iter().for_each(|(key, stroke)| {
            // Blur debug rendering for strokes which are normally hidden
            if let (Some(render_comp), Some(trash_comp)) = (
                self.render_components.get(key),
                self.trash_components.get(key),
            ) {
                if render_comp.render && trash_comp.trashed {
                    snapshot.push_blur(3.0);
                    snapshot.push_opacity(0.2);
                }
            }
            match stroke {
                StrokeStyle::MarkerStroke(markerstroke) => {
                    for element in markerstroke.elements.iter() {
                        canvas::debug::draw_pos(
                            element.inputdata.pos(),
                            canvas::debug::COLOR_POS,
                            scalefactor,
                            snapshot,
                        )
                    }
                    for &hitbox_elem in markerstroke.hitbox.iter() {
                        canvas::debug::draw_bounds(
                            hitbox_elem,
                            canvas::debug::COLOR_STROKE_HITBOX,
                            scalefactor,
                            snapshot,
                        );
                    }
                    canvas::debug::draw_bounds(
                        markerstroke.bounds,
                        canvas::debug::COLOR_STROKE_BOUNDS,
                        scalefactor,
                        snapshot,
                    );
                }
                StrokeStyle::BrushStroke(brushstroke) => {
                    for element in brushstroke.elements.iter() {
                        canvas::debug::draw_pos(
                            element.inputdata.pos(),
                            canvas::debug::COLOR_POS,
                            scalefactor,
                            snapshot,
                        )
                    }
                    for &hitbox_elem in brushstroke.hitbox.iter() {
                        canvas::debug::draw_bounds(
                            hitbox_elem,
                            canvas::debug::COLOR_STROKE_HITBOX,
                            scalefactor,
                            snapshot,
                        );
                    }
                    canvas::debug::draw_bounds(
                        brushstroke.bounds,
                        canvas::debug::COLOR_STROKE_BOUNDS,
                        scalefactor,
                        snapshot,
                    );
                }
                StrokeStyle::ShapeStroke(shapestroke) => {
                    canvas::debug::draw_bounds(
                        shapestroke.bounds,
                        canvas::debug::COLOR_STROKE_BOUNDS,
                        scalefactor,
                        snapshot,
                    );
                }
                StrokeStyle::VectorImage(vectorimage) => {
                    canvas::debug::draw_bounds(
                        vectorimage.bounds,
                        canvas::debug::COLOR_STROKE_BOUNDS,
                        scalefactor,
                        snapshot,
                    );
                }
                StrokeStyle::BitmapImage(bitmapimage) => {
                    canvas::debug::draw_bounds(
                        bitmapimage.bounds,
                        canvas::debug::COLOR_STROKE_BOUNDS,
                        scalefactor,
                        snapshot,
                    );
                }
            }
            if let (Some(render_comp), Some(trash_comp)) = (
                self.render_components.get(key),
                self.trash_components.get(key),
            ) {
                if render_comp.render && trash_comp.trashed {
                    snapshot.pop();
                    snapshot.pop();
                }
            }
        });
    }
}
