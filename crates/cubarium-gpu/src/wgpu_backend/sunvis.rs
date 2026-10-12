//! Safe sun-volume uploads retain the staging/record/retirement ownership rule.
#![forbid(unsafe_code)]
use super::Gpu;
use anyhow::Result;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Upload {
    Idle,
    Owed,
    Recorded(usize),
}
pub struct SunVisVolume {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    extent: wgpu::Extent3d,
    staging: Option<Vec<u8>>,
    upload: Upload,
    fade: f32,
}
impl SunVisVolume {
    pub fn new(gpu: &Gpu, on: bool, width: u32, height: u32, depth: u32) -> Self {
        let extent = if on {
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            }
        } else {
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            }
        };
        let texture = gpu.texture(
            "sun visibility",
            extent,
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::Rg8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            extent,
            staging: on.then(|| vec![0; (width * height * depth * 2) as usize]),
            upload: if on { Upload::Owed } else { Upload::Idle },
            fade: 1.0,
        }
    }
    pub fn on(&self) -> bool {
        self.staging.is_some()
    }
    pub fn bytes(&self) -> usize {
        (self.extent.width * self.extent.height * self.extent.depth_or_array_layers * 2) as usize
    }
    pub fn accepts(&self) -> bool {
        self.on() && !matches!(self.upload, Upload::Recorded(_))
    }
    pub fn put(&mut self, rg: &[u8], fade: f32) -> bool {
        if !self.accepts() || rg.len() != self.bytes() {
            return false;
        }
        self.staging.as_mut().unwrap().copy_from_slice(rg);
        self.upload = Upload::Owed;
        self.fade = fade.clamp(0.0, 1.0);
        true
    }
    pub fn set_fade(&mut self, fade: f32) -> bool {
        let fade = fade.clamp(0.0, 1.0);
        if self.upload == Upload::Owed
            || (fade * 1024.0).round() as i32 == (self.fade * 1024.0).round() as i32
        {
            return false;
        }
        self.fade = fade;
        true
    }
    pub fn fade(&self) -> f32 {
        self.fade
    }
    pub fn owed(&self) -> bool {
        self.upload == Upload::Owed
    }
    pub fn record_upload(
        &mut self,
        upload: &mut super::upload::Upload<'_>,
        encoder: &mut wgpu::CommandEncoder,
        slot: usize,
    ) -> Result<()> {
        if self.upload == Upload::Owed {
            upload.texture(
                encoder,
                &self.texture,
                wgpu::Origin3d::ZERO,
                self.extent,
                2,
                self.staging.as_ref().unwrap(),
            )?;
            self.upload = Upload::Recorded(slot);
        }
        Ok(())
    }
    pub fn retired(&mut self, slot: usize) {
        if self.upload == Upload::Recorded(slot) {
            self.upload = Upload::Idle;
        }
    }
    pub fn discarded(&mut self, slot: usize) {
        if self.upload == Upload::Recorded(slot) {
            self.upload = Upload::Owed;
        }
    }
}
