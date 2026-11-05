/**
 * "Caratteristiche" | "In tempo reale" split inside a hardware card. Responds to the page
 * container (`@container` in `HardwarePage`), not the window: two columns only when the
 * content is at least 768 px wide, so long labels never squeeze the values.
 */
export const SPLIT = "grid grid-cols-1 gap-x-8 gap-y-6 @3xl:grid-cols-2";
