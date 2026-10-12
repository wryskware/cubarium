//! Legacy-shaped synchronous headless target on the safe frame interface.
#![forbid(unsafe_code)]

use std::time::Duration;

use anyhow::{Result, ensure};

use super::{FrameSource, Gpu, PresentPass, ReadbackRing};

struct Output {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    present: PresentPass,
    transform: [f32; 8],
}

pub struct Headless {
    output: Option<Output>,
    closed: bool,
}

impl Headless {
    pub fn new<S: FrameSource>(_gpu: &Gpu, _source: &S) -> Result<Self> {
        Ok(Self {
            output: None,
            closed: false,
        })
    }

    pub fn with_size<S: FrameSource>(gpu: &Gpu, source: &S, size: (u32, u32)) -> Result<Self> {
        ensure!(
            size.0 > 0 && size.1 > 0,
            "headless output must have positive dimensions"
        );
        let texture = gpu.texture(
            "headless presentation",
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let view = texture.create_view(&Default::default());
        let present = PresentPass::new(gpu, texture.format());
        let transform = crate::present::WindowFit::fit(source.raster_size(), size, false).push();
        Ok(Self {
            output: Some(Output {
                texture,
                view,
                present,
                transform,
            }),
            closed: false,
        })
    }

    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        source: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<f64> {
        ensure!(!self.closed, "headless target is closed");
        let mut encoder = gpu.encoder("headless frame");
        source.record_frame(gpu, &mut encoder, frame, None)?;
        if let Some(output) = &self.output {
            output.present.record(
                gpu,
                &mut encoder,
                source.raster_view(),
                &output.view,
                output.transform,
            );
        }
        let submission = gpu.queue.submit([encoder.finish()]);
        gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })?;
        source.frame_retired();
        Ok(source.gpu_ms(gpu))
    }

    pub fn read<S: FrameSource>(&self, gpu: &Gpu, source: &S) -> Result<Vec<u8>> {
        ensure!(!self.closed, "headless target is closed");
        source.read_raster(gpu)
    }

    pub fn read_output<S: FrameSource>(&self, gpu: &Gpu, source: &S) -> Result<Vec<u8>> {
        ensure!(!self.closed, "headless target is closed");
        match &self.output {
            Some(output) => read_texture(gpu, &output.texture),
            None => source.read_raster(gpu),
        }
    }

    /// Output handles are reference counted by wgpu; completed draws leave no
    /// target-owned submission to drain. Release the optional output immediately.
    pub fn destroy(&mut self, _gpu: &Gpu) {
        self.output.take();
        self.closed = true;
    }
}

pub fn read_texture(gpu: &Gpu, texture: &wgpu::Texture) -> Result<Vec<u8>> {
    let mut ring = ReadbackRing::new(gpu, (texture.width(), texture.height()), 1)?;
    ring.submit(gpu, gpu.encoder("headless readback"), texture, 0)?;
    let mut bytes = Vec::new();
    ring.drain(gpu, |_, pixels| bytes.extend_from_slice(pixels))?;
    Ok(bytes)
}
