//! 3DS Screen Rendering Module
//!
//! This module handles rendering of the Nintendo 3DS dual-screen display using winit for
//! window management and softbuffer for software rendering.

use crate::core::EmulatorCore;
use crate::scheduler::QuantumResult;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use tracing::info;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

// ================================================================================================
// Screen Dimension Constants
// ================================================================================================

/// Width of the top screen in pixels (wider screen)
const TOP_SCREEN_WIDTH: u32 = 400;

/// Height of the top screen in pixels
const TOP_SCREEN_HEIGHT: u32 = 240;

/// Width of the bottom screen in pixels (touchscreen)
const BOTTOM_SCREEN_WIDTH: u32 = 320;

/// Height of the bottom screen in pixels
const BOTTOM_SCREEN_HEIGHT: u32 = 240;

// ================================================================================================
// Window Layout Constants
// ================================================================================================

/// Border size around the screens in pixels
const BORDER_SIZE: u32 = 4;

/// Gap between top and bottom screens in pixels
const SCREEN_GAP: u32 = 4;

/// Total window width including borders
const WINDOW_WIDTH: u32 = TOP_SCREEN_WIDTH + (BORDER_SIZE * 2);

/// Total window height including both screens, gap, and borders
const WINDOW_HEIGHT: u32 =
    TOP_SCREEN_HEIGHT + BOTTOM_SCREEN_HEIGHT + SCREEN_GAP + (BORDER_SIZE * 2);

/// X coordinate of top screen within the window (accounting for left border)
const TOP_SCREEN_X: u32 = BORDER_SIZE;

/// Y coordinate of top screen within the window (accounting for top border)
const TOP_SCREEN_Y: u32 = BORDER_SIZE;

/// X coordinate of bottom screen within the window (centered horizontally)
const BOTTOM_SCREEN_X: u32 = BORDER_SIZE + (TOP_SCREEN_WIDTH - BOTTOM_SCREEN_WIDTH) / 2;

/// Y coordinate of bottom screen within the window (below top screen + gap)
const BOTTOM_SCREEN_Y: u32 = BORDER_SIZE + TOP_SCREEN_HEIGHT + SCREEN_GAP;

/// Border color in RGB format (dark grey: 0x333333)
const BORDER_COLOR: u32 = 0x333333;

// ================================================================================================
// Framebuffer Format Constants
// ================================================================================================

/// Number of bytes per pixel in RGB8 format (Red, Green, Blue)
const BYTES_PER_PIXEL_RGB8: u32 = 3;

// ================================================================================================
// Memory Address Range Constants
// ================================================================================================

/// Base address of VRAM (Video RAM) - 6 MB region
const VRAM_BASE: u32 = 0x18000000;

/// End address of VRAM (exclusive)
const VRAM_END: u32 = 0x18600000;

/// Base address of FCRAM (Fast Cycle RAM) - 128 MB region
const FCRAM_BASE: u32 = 0x20000000;

// ================================================================================================
// Display Timing Constants
// ================================================================================================

/// Number of emulation quanta per frame
const QUANTUMS_PER_FRAME: usize = 10;

/// Emulator display application
pub struct EmulatorDisplay {
    emulator: EmulatorCore,

    // Display state
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,

    quantums_completed_in_this_frame: usize,
}

impl EmulatorDisplay {
    pub fn new(emulator: EmulatorCore) -> Self {
        Self {
            emulator,
            window: None,
            surface: None,
            quantums_completed_in_this_frame: 0,
        }
    }
}

impl ApplicationHandler for EmulatorDisplay {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Rc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("threemu")
                        .with_inner_size(winit::dpi::PhysicalSize::new(
                            WINDOW_WIDTH,
                            WINDOW_HEIGHT,
                        )),
                )
                .unwrap(),
        );

        let context = Context::new(window.clone()).unwrap();
        let mut surface = Surface::new(&context, window.clone()).unwrap();
        surface
            .resize(
                NonZeroU32::new(WINDOW_WIDTH).unwrap(),
                NonZeroU32::new(WINDOW_HEIGHT).unwrap(),
            )
            .unwrap();

        self.window = Some(window.clone());
        self.surface = Some(surface);

        // Kick off the first frame
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                info!("=== Emulation Stopped ===");
                self.emulator.print_final_state();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(surface) = self.surface.as_mut() {
                    Self::render(surface, &self.emulator);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Run a quantum
        let result = self.emulator.step();

        // Check stop conditions
        let should_stop = matches!(result, QuantumResult::Error(_)) || self.emulator.should_stop();

        if should_stop {
            info!("=== Stop Condition Reached ===");
            self.emulator.print_final_state();
            event_loop.exit();
            return;
        }

        self.quantums_completed_in_this_frame += 1;
        if self.quantums_completed_in_this_frame >= QUANTUMS_PER_FRAME
            && let Some(window) = self.window.as_mut()
        {
            window.request_redraw();
            self.quantums_completed_in_this_frame = 0;
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }
}

impl EmulatorDisplay {
    fn render(surface: &mut Surface<Rc<Window>, Rc<Window>>, emulator: &EmulatorCore) {
        let mut buffer = surface.buffer_mut().unwrap();

        // Fill with border color
        for pixel in buffer.iter_mut() {
            *pixel = BORDER_COLOR;
        }

        // Get GPU state from ARM11
        let gpu_state = &emulator.arm11_emu().get_data().gpu;

        // Get memory buffers
        let fcram = emulator.fcram();
        let vram = emulator.vram();

        // Render top screen if we have an address
        if gpu_state.top_left_addr != 0 {
            Self::render_screen(
                &mut buffer,
                fcram,
                vram,
                gpu_state.top_left_addr,
                TOP_SCREEN_X,
                TOP_SCREEN_Y,
                TOP_SCREEN_WIDTH,
                TOP_SCREEN_HEIGHT,
            );
        }

        // Render bottom screen if we have an address
        if gpu_state.bottom_addr != 0 {
            Self::render_screen(
                &mut buffer,
                fcram,
                vram,
                gpu_state.bottom_addr,
                BOTTOM_SCREEN_X,
                BOTTOM_SCREEN_Y,
                BOTTOM_SCREEN_WIDTH,
                BOTTOM_SCREEN_HEIGHT,
            );
        }

        buffer.present().unwrap();
    }

    /// Renders a 3DS screen framebuffer to the display buffer with 90° rotation
    #[expect(clippy::too_many_arguments)]
    fn render_screen(
        buffer: &mut [u32],
        fcram: &[u8],
        vram: &[u8],
        fb_addr: u32,
        screen_x: u32,
        screen_y: u32,
        width: u32,
        height: u32,
    ) {
        // Iterate over each pixel in the screen's display coordinates
        for screen_y_offset in 0..height {
            for screen_x_offset in 0..width {
                // The 3DS framebuffer is stored rotated 90° counter-clockwise from the display.
                // To render correctly, we need to rotate 90° clockwise when reading.
                let fb_x = height - 1 - screen_y_offset;
                let fb_y = screen_x_offset;

                // Calculate pixel address in framebuffer using the rotated coordinates
                let pixel_addr = fb_addr + ((fb_y * height + fb_x) * BYTES_PER_PIXEL_RGB8);

                // Read pixel data from the appropriate memory region based on address
                let (r, g, b) = if (VRAM_BASE..VRAM_END).contains(&pixel_addr) {
                    // VRAM region: 0x18000000 - 0x18600000 (6 MB)
                    let vram_offset = (pixel_addr - VRAM_BASE) as usize;
                    if vram_offset + 2 < vram.len() {
                        (
                            vram[vram_offset] as u32,
                            vram[vram_offset + 1] as u32,
                            vram[vram_offset + 2] as u32,
                        )
                    } else {
                        (0, 0, 0)
                    }
                } else if pixel_addr >= FCRAM_BASE {
                    // FCRAM region: 0x20000000+ (128 MB)
                    let fcram_offset = (pixel_addr - FCRAM_BASE) as usize;
                    if fcram_offset + 2 < fcram.len() {
                        (
                            fcram[fcram_offset] as u32,
                            fcram[fcram_offset + 1] as u32,
                            fcram[fcram_offset + 2] as u32,
                        )
                    } else {
                        (0, 0, 0)
                    }
                } else {
                    // Invalid address - render as black
                    (0, 0, 0)
                };

                // Calculate position in the output window buffer
                let window_x = screen_x + screen_x_offset;
                let window_y = screen_y + screen_y_offset;
                let idx = (window_y * WINDOW_WIDTH + window_x) as usize;

                // Write pixel to output buffer in 0xRRGGBB format
                if idx < buffer.len() {
                    buffer[idx] = (r << 16) | (g << 8) | b;
                }
            }
        }
    }
}

pub fn run(emulator: EmulatorCore) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = EmulatorDisplay::new(emulator);
    event_loop.run_app(&mut app)?;
    Ok(())
}
