import type { ComponentType } from "react";
import type { DeviceKind } from "../types";
import { AllInOne } from "./AllInOne";
import { Laptop } from "./Laptop";
import { MiniPc } from "./MiniPc";
import { Tower } from "./Tower";

/** The 3D model of every device kind (lazy chunk only). A new kind is listed here. */
export const MODELS: Record<DeviceKind, ComponentType> = {
  tower: Tower,
  laptop: Laptop,
  mini: MiniPc,
  allInOne: AllInOne,
};
