# ASIMOV Camera Module

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Package on Crates.io](https://img.shields.io/crates/v/asimov-camera-module)](https://crates.io/crates/asimov-camera-module)
[![Documentation](https://docs.rs/asimov-camera-module/badge.svg)](https://docs.rs/asimov-camera-module)

An [ASIMOV] module that streams camera frames via **FFmpeg** and emits one **JSON-LD `Image`** per frame to **stdout**.

## ✨ Features

- To be determined!

## 🛠️ Prerequisites

- [Rust] 1.85+ (2024 edition)
- **FFmpeg** installed and on `PATH` (`ffmpeg` must be callable)

## ⬇️ Installation

### Installation with the [ASIMOV CLI]

```bash
asimov module install camera -v
```

### Installation from Source Code

```bash
cargo install asimov-camera-module
```

## 👉 Examples

**Basic camera stream**
```bash
asimov-camera-reader
```
(`file:/dev/video0` is used by default.)

**Enumerate cameras (text)**
```bash
# Short summary (IDs + human names)
asimov-camera-cataloger

# Verbose (descriptions, misc info, resolutions, frame rates)
asimov-camera-cataloger -v
```

**Enumerate cameras (JSONL)**
```bash
# JSONL with logical IDs and formats
asimov-camera-cataloger --output jsonl

# Pipe into jq
asimov-camera-cataloger --output jsonl | jq .
```
Then take the `id` from the cataloger output and plug it into the reader:
```bash
# Using a discovered device, e.g. "file:/dev/video2"
asimov-camera-reader file:/dev/video2
```

## ⚙ Configuration

This module requires no configuration.

## 📚 Reference

### Installed Binaries

- `asimov-camera-reader` — streams camera frames as JSONL KNOW Image objects.
- `asimov-camera-cataloger` — lists available camera devices and their supported formats.

### `asimov-camera-reader`

```
Usage: asimov-camera-reader [OPTIONS] [device]

Arguments:
  [device]  Input camera device (default: file:/dev/video0)

Options:
  -s, --size <WxH>      Desired dimensions (e.g. 640x480, 1920x1080) [default: 640x480]
  -f, --frequency <Hz>  Sampling frequency in Hz (frames per second) [default: 30]
  -D, --debounce...     Debounce level (repeat flag to increase threshold)
  -d, --debug           Enable debugging output
      --license         Show license information
  -v, --verbose...      Enable verbose output (repeat for more verbosity)
  -V, --version         Print version information
  -h, --help            Print help
```

### Device examples

**macOS (avfoundation)**

Use `0`, `1`, … (the module maps `file:/dev/videoN` → `N`)
```bash
asimov-camera-reader 0
asimov-camera-reader --size 1280x720 --frequency 15 0
```

**Linux (v4l2)**
```bash
asimov-camera-reader 0
asimov-camera-reader file:/dev/video2 -s 1920x1080 -f 30
```

**Windows (dshow)**
```bash
asimov-camera-reader "video=Integrated Camera"
```

### Debounce
Each `-D` raises the Hamming-distance threshold (perceptual hash):
```bash
asimov-camera-reader -D        # low debounce
asimov-camera-reader -DDD      # stricter
```

### `asimov-camera-cataloger`

```
Usage: asimov-camera-cataloger [OPTIONS]

Options:
  -o, --output <FORMAT>  Output format [default: text] [possible values: text, jsonl]
  -d, --debug            Enable debugging output
      --license          Show license information
  -v, --verbose...       Enable verbose output (repeat for more verbosity)
  -V, --version          Print version information
  -h, --help             Print help
```

**Text output**
```
asimov-camera-cataloger
# file:/dev/video0: Integrated Camera
# file:/dev/video1: USB Camera

asimov-camera-cataloger -v
# file:/dev/video0: Integrated Camera
#   <description>
#   <misc>
#   Available formats:
#       Resolution 640x480
#           Frame rate: 30 fps
#           Frame rate: 60 fps
#       Resolution 1280x720
#           Frame rate: 30 fps
```

**JSONL output**
```bash
asimov-camera-cataloger --output jsonl | jq .
```
Each line is a single device:
```json
{
  "id": "file:/dev/video0",
  "name": "Integrated Camera",
  "description": "Built-in iSight",
  "misc": "…",
  "formats": [
    {
      "width": 640,
      "height": 480,
      "frame_rates": [
        { "value": "30" },
        { "value": "60" }
      ]
    }
  ]
}
```
Use the `id` field with `asimov-camera-reader`.

## Output ([JSON-LD] Image)

### JSONL

One JSON object per line:
```json
{
  "@type": "Image",
  "@id": "file:/dev/video0#1763041205",
  "width": 640,
  "height": 480,
  "source": "file:/dev/video0",
  "data": "data:image/rgb;base64,AAAA..."
}
```
> [!NOTE]
> Note that the image data must be the uncompressed raw 24-bit RGB data,
> Base64-encoded into a `data:image/rgb;base64,...` URL.

## 👨‍💻 Development

```bash
git clone https://github.com/asimov-modules/asimov-camera-module.git
```

[![Share on X](https://img.shields.io/badge/share%20on-x-03A9F4?logo=x)](https://x.com/intent/post?url=https://github.com/asimov-modules/asimov-camera-module&text=asimov-camera-module)
[![Share on Reddit](https://img.shields.io/badge/share%20on-reddit-red?logo=reddit)](https://reddit.com/submit?url=https://github.com/asimov-modules/asimov-camera-module&title=asimov-camera-module)
[![Share on Hacker News](https://img.shields.io/badge/share%20on-hn-orange?logo=ycombinator)](https://news.ycombinator.com/submitlink?u=https://github.com/asimov-modules/asimov-camera-module&t=asimov-camera-module)
[![Share on Facebook](https://img.shields.io/badge/share%20on-fb-1976D2?logo=facebook)](https://www.facebook.com/sharer/sharer.php?u=https://github.com/asimov-modules/asimov-camera-module)
[![Share on LinkedIn](https://img.shields.io/badge/share%20on-linkedin-3949AB?logo=linkedin)](https://www.linkedin.com/sharing/share-offsite/?url=https://github.com/asimov-modules/asimov-camera-module)

[ASIMOV]: https://asimov.sh
[ASIMOV CLI]: https://cli.asimov.sh
[JSON-LD]: https://json-ld.org
[KNOW]: https://know.dev
[Rust]: https://rust-lang.org
