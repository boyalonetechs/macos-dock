#  macOS Tahoe Liquid Glass Dock (Rust)

A high-performance, open-source macOS-inspired Dock built from the ground up in **Rust**, faithfully recreating the futuristic visual identity of **macOS Tahoe**. Engineered for minimal resource overhead, silk-smooth 60+ FPS animations, and deep native OS integration without the heavy memory footprint of Electron or web-based desktop wrappers.

---

## ✨ Features

- **Blazing Fast Performance:** Written in pure Rust for near-instant startup, negligible CPU usage, and low double-digit RAM footprint.
- **macOS Tahoe Liquid Glass Aesthetic:** 
  - Dynamic visual refraction and real-time frosted glassmorphism (`backdrop-filter`).
  - Fluid light-dispersion highlights that respond to cursor proximity.
  - Layered translucent depth effects with system wallpaper color sampling.
  - Smooth spring-physics icon magnification and parabolic scaling.
- **Native System Integration:**
  - Active application tracking with liquid indicator dots.
  - Window switching, minimizing, and desktop workspace integration.
  - Shell application launcher with custom desktop entry parser.
- **Customization & Themes:** Fully configurable auto-hide behavior, position pinning (bottom, left, right), icon padding, and custom app shortcuts via JSON/TOML configuration.
- **Zero Web Runtime Overhead:** No Chromium, webviews, or heavy JavaScript engines running in the background.

---

## 🛠️ Tech Stack & Dependencies

- **Language:** [Rust](https://www.rust-lang.org/) (2021 edition)
- **GUI & Graphics:** [`wgpu`](https://github.com/gfx-rs/wgpu) / Custom Metal Shader Pipeline
- **Windowing:** [`winit`](https://github.com/rust-windowing/winit)
- **Physics Engine:** Custom spring-damper spring physics for layout transitions
- **Configuration:** [`serde`](https://github.com/serde-rs/serde) + [`toml`](https://github.com/toml-rs/toml)

---

## 📦 Installation

### Prerequisites

Ensure you have Rust and `cargo` installed on your system:

```bash
curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
