# Abstract Art

Abstract Art creates reproducible abstract images from mathematical patterns and seeded randomness. This vocabulary describes the choices that define an image and the frames presented as output.

## Language

**Style**:
A named visual construction, such as layered waves, glowing curves, or mountain silhouettes. A style and a palette are separate choices, even when they share a name such as `ember`.
_Avoid_: effect, preset (when referring to a style alone)

**Palette**:
A named, ordered color progression defined by palette stops, with intermediate colors between adjacent stops. A palette is a color choice available to every style.
_Avoid_: theme, color scheme

**Palette stop**:
An anchor color in a palette's ordered progression. A stop is a color, rather than a boundary between regions of an image.
_Avoid_: color band

**Canvas**:
The rectangular area of an image, whose width and height are measured in pixels. Its dimensions can affect the scene's arrangement as well as the amount of visible detail.

**Seed**:
An integer controlling random choices and noise; the same seed, style, palette, and canvas dimensions give repeatable results within a fixed version and environment. Different seeds need not produce different images; the Mandelbrot style is independent of the seed.
_Avoid_: image ID

**Scene**:
The arrangement and visual parameters of shapes, curves, or fields for one generated image, before they become a rendered frame.
_Avoid_: composition (when referring to this arrangement), frame

**Frame**:
One complete rendered image covering the canvas, whether presented as a still image or as part of an animation. Identical images can occupy separate positions in an animation.
_Avoid_: scene (when referring to the rendered image)

**Phase**:
The position in a loop, from 0 up to 1, that every style uses to move its scene. Phase 1.0 looks the same as phase 0.0, and phase 0 gives the style's still image.
_Avoid_: time, frame number

**Animation**:
A looping sequence of complete frames with the same canvas dimensions and a shared playback rate. Every style has motion, so the frames show one scene at successive phases. Written as APNG or, when the output name ends in `.gif`, as a GIF.
_Avoid_: simulation

**Slideshow**:
An animation whose frames are unrelated scenes with consecutive seeds, requested with `--slideshow`. It is what a style without motion would produce under `--frames`.
_Avoid_: animation (when the frames are independent scenes)

**Still image**:
A single frame, written as PNG (or GIF when the output name ends in `.gif`). With a `--phase`, it shows that moment of the style's loop.

**Color table**:
The at most 256 colors a GIF can use. All frames of one GIF share a table sampled along the palette, plus greys for glows, so gradients can band slightly. PNG and APNG have no such limit.
_Avoid_: palette (a palette is the named color progression a style draws from)
