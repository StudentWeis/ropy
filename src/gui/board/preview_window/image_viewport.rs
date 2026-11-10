//! Raster geometry in window-local logical pixels, independent of UI spacing.
use gpui_kit::{Bounds, Pixels, Point, Size, point, px};

#[derive(Clone, Copy, Default)]
pub(super) struct ImageViewport {
    bounds: Bounds<Pixels>,
    dimensions: Size<Pixels>,
    /// Relative to fit-to-viewport, not original image pixels.
    zoom: f32,
    offset: Point<Pixels>,
}

impl ImageViewport {
    pub(super) fn set_bounds(&mut self, bounds: Bounds<Pixels>) {
        self.bounds = bounds;
        self.clamp_offset();
    }

    pub(super) fn set_dimensions(&mut self, dimensions: Size<Pixels>) {
        self.dimensions = dimensions;
        if self.zoom == 0. {
            self.zoom = 1.;
        }
        self.clamp_offset();
    }

    fn fit_scale(self) -> f32 {
        if self.dimensions.width <= px(0.) || self.dimensions.height <= px(0.) {
            return 1.;
        }
        (self.bounds.size.width / self.dimensions.width)
            .min(self.bounds.size.height / self.dimensions.height)
            .min(1.)
    }

    pub(super) fn scale(self) -> f32 {
        self.fit_scale() * self.zoom
    }

    pub(super) fn placement(self) -> Bounds<Pixels> {
        let extent = self.dimensions.map(|edge| edge * self.scale());
        Bounds::new(
            point(
                (self.bounds.size.width - extent.width) / 2.,
                (self.bounds.size.height - extent.height) / 2.,
            ) + self.offset,
            extent,
        )
    }

    pub(super) const fn offset(self) -> Point<Pixels> {
        self.offset
    }

    pub(super) fn reset(&mut self) {
        self.zoom = 1.;
        self.offset = Point::default();
    }

    pub(super) fn original_size(&mut self) {
        let fit = self.fit_scale();
        if fit > 0. {
            self.zoom = 1. / fit;
        }
        self.offset = Point::default();
    }

    pub(super) fn zoom_at(&mut self, pointer: Point<Pixels>, delta: f32) {
        if self.zoom <= 0. || !delta.is_finite() {
            return;
        }
        let previous = self.zoom;
        // Always allow original pixels, even for images much larger than the screen.
        let maximum = 8_f32.max(1. / self.fit_scale().max(0.0001));
        self.zoom = (previous * (delta * 0.002).exp()).clamp(0.1, maximum);
        let anchor = pointer - self.bounds.center();
        self.offset = anchor - (anchor - self.offset) * (self.zoom / previous);
        self.clamp_offset();
    }

    pub(super) fn pan(&mut self, offset: Point<Pixels>) {
        self.offset = offset;
        self.clamp_offset();
    }

    fn clamp_offset(&mut self) {
        let extent = self.placement().size;
        let limit_x = ((extent.width - self.bounds.size.width) / 2.).max(px(0.));
        let limit_y = ((extent.height - self.bounds.size.height) / 2.).max(px(0.));
        self.offset.x = self.offset.x.clamp(-limit_x, limit_x);
        self.offset.y = self.offset.y.clamp(-limit_y, limit_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::size;

    fn viewport() -> ImageViewport {
        let mut image = ImageViewport::default();
        image.set_bounds(Bounds::new(
            point(px(20.), px(60.)),
            size(px(800.), px(600.)),
        ));
        image.set_dimensions(size(px(1600.), px(800.)));
        image
    }

    #[test]
    fn test_image_fit_preserves_aspect_ratio_and_pan_stops_at_edges() {
        let mut image = viewport();
        assert_eq!(
            image.placement(),
            Bounds::new(point(px(0.), px(100.)), size(px(800.), px(400.)))
        );
        image.pan(point(px(500.), px(-500.)));
        assert_eq!(image.offset(), Point::default());
        image.original_size();
        image.pan(point(px(9999.), px(-9999.)));
        assert_eq!(
            image.placement(),
            Bounds::new(point(px(0.), px(-200.)), size(px(1600.), px(800.)))
        );
        image.reset();
        assert_eq!(image.offset(), Point::default());
    }

    #[test]
    fn test_image_zoom_keeps_pointer_anchor_and_shrinking_recenters() {
        let mut image = viewport();
        image.original_size();
        let pointer = point(px(520.), px(410.));
        let before = image.placement();
        let relative = pointer - image.bounds.origin - before.origin;
        image.zoom_at(pointer, 100.);
        let after = image.placement();
        let actual = pointer - image.bounds.origin - after.origin;
        let expected = relative * (after.size.width / before.size.width);
        assert!((actual.x - expected.x).abs() < px(0.01));
        assert!((actual.y - expected.y).abs() < px(0.01));
        image.zoom_at(pointer, -10000.);
        assert_eq!(image.offset(), Point::default());
    }
}
