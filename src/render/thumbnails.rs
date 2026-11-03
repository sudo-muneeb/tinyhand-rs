use super::*;
impl Renderer {
    pub(super) unsafe fn custom_thumbnail(
        &mut self,
        c: *mut Cairo,
        id: &str,
        x: f64,
        y: f64,
        size: f64,
    ) {
        if !self.custom_previews.contains_key(id) {
            if self.custom_previews.len() >= 16 {
                for (_, image) in self.custom_previews.drain() {
                    cairo_surface_destroy(image);
                }
            }
            let path = crate::hands::Library::path(id);
            let Ok(path) = CString::new(path.to_string_lossy().as_bytes()) else {
                return;
            };
            let full = cairo_image_surface_create_from_png(path.as_ptr());
            if cairo_surface_status(full) != 0 {
                cairo_surface_destroy(full);
                return;
            }
            let small = cairo_image_surface_create(0, 64, 96);
            let context = cairo_create(small);
            cairo_scale(
                context,
                64.0 / cairo_image_surface_get_width(full) as f64,
                96.0 / cairo_image_surface_get_height(full) as f64,
            );
            cairo_set_source_surface(context, full, 0.0, 0.0);
            cairo_paint(context);
            cairo_destroy(context);
            cairo_surface_destroy(full);
            self.custom_previews.insert(id.to_owned(), small);
        }
        let image = self.custom_previews[id];
        cairo_save(c);
        cairo_rectangle(c, x, y, size, size);
        cairo_clip(c);
        cairo_translate(c, x, y);
        let scale = size / (96.0 * 0.49);
        cairo_scale(c, scale, scale);
        cairo_set_source_surface(c, image, -32.0 + size / scale * 0.5, 0.0);
        cairo_paint(c);
        cairo_restore(c);
    }
    pub(super) unsafe fn thumbnail(
        &mut self,
        c: *mut Cairo,
        index: usize,
        x: f64,
        y: f64,
        size: f64,
    ) {
        if index == 36 {
            cairo_new_path(c);
            cairo_set_source_rgba(c, colors::RED.0, colors::RED.1, colors::RED.2, 1.0);
            cairo_arc(
                c,
                x + size / 2.0,
                y + size / 2.0,
                5.0,
                0.0,
                std::f64::consts::TAU,
            );
            cairo_fill(c);
            return;
        }
        if self.previews.is_empty() {
            self.previews.resize(36, std::ptr::null_mut());
        }
        if self.previews[index].is_null() {
            let style = STYLES[index];
            let name = CString::new(
                self.asset_dir
                    .join(format!("hand_{style}.png"))
                    .to_string_lossy()
                    .as_bytes(),
            )
            .unwrap();
            let full = cairo_image_surface_create_from_png(name.as_ptr());
            let small = cairo_image_surface_create(0, 64, 96);
            let context = cairo_create(small);
            let sx = 64.0 / cairo_image_surface_get_width(full) as f64;
            let sy = 96.0 / cairo_image_surface_get_height(full) as f64;
            cairo_scale(context, sx, sy);
            cairo_set_source_surface(context, full, 0.0, 0.0);
            cairo_paint(context);
            cairo_destroy(context);
            cairo_surface_destroy(full);
            self.previews[index] = small;
        }
        let image = self.previews[index];
        if cairo_surface_status(image) != 0 {
            return;
        }
        let w = cairo_image_surface_get_width(image) as f64;
        let h = cairo_image_surface_get_height(image) as f64;
        // Crop the rod out of the selection thumbnail, as upstream does.
        cairo_save(c);
        cairo_rectangle(c, x, y, size, size);
        cairo_clip(c);
        cairo_translate(c, x, y);
        let scale = size / (h * 0.49);
        cairo_scale(c, scale, scale);
        cairo_set_source_surface(c, image, -w * 0.5 + size / scale * 0.5, 0.0);
        cairo_paint(c);
        cairo_restore(c);
    }
}
