import type it from "../it/device3d";
import type { Translation } from "../types";

/** English texts for the "device3d" namespace (the 3D device stage). */
export default {
  model_tower: "3D model: tower desktop PC",
  model_laptop: "3D model: laptop",
  model_mini: "3D model: mini PC",
  illustration_tower: "Illustration: tower desktop PC",
  illustration_laptop: "Illustration: laptop",
  illustration_mini: "Illustration: mini PC",
  rotateHint: "Drag or use the arrow keys to rotate. Home restores the starting view.",
  "rotateHint@macos": "Drag or use the arrow keys to rotate. Fn+← restores the starting view.",
  loading: "Loading the 3D model",
  noWebgl: "3D graphics are not available in this window: an illustration is shown.",
  contextLost: "3D graphics stopped: an illustration is shown.",
} satisfies Translation<typeof it>;
