pub trait Painter {
    fn name(&self) -> &'static str;

    fn resize_buffers<F, R>(&mut self, callback: F) -> anyhow::Result<R>
    where
        F: FnOnce() -> R;

    fn set_texture(&mut self, tex_id: u64, image: epi::Image);

    fn free_texture(&mut self, tex_id: u64);

    fn debug_info(&self) -> String;

    fn upload_egui_texture(&mut self, font_image: &egui::FontImage);

    fn paint_meshes(&mut self, clipped_meshes: Vec<egui::ClippedMesh>, pixels_per_point: f32) -> anyhow::Result<()>;
}
