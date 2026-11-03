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

**Animation**:
A looping sequence of complete frames with the same canvas dimensions and a shared playback rate. Frames may show independently generated scenes; an animation does not imply continuous motion between them.
_Avoid_: simulation
